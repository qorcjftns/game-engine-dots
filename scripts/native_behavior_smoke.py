"""M4 실제 native host + CLI/MCP + 독립 player 회귀. 물리 키 입력은 별도 수동 QA다."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from native_session_smoke import McpClient, SUFFIX
from native_runtime_smoke import assert_native_pixels
from smoke_readonly import png_pixel

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / 'target/debug'
SOURCE = 'if right { move(2,0); }\nif tick <= 40 { move(1,0); }\nset_state(state+1);\nif tick == 1 { log("started"); }'
EMPTY = dict(left=False, right=False, up=False, down=False, primary=False, pointer=None)


def main():
    directory = ROOT / 'artifacts/native-behavior'
    directory.mkdir(parents=True, exist_ok=False)
    project_dir = directory / 'project'
    (project_dir / 'assets').mkdir(parents=True)
    (project_dir / 'assets/pixel.png').write_bytes(png_pixel())
    project = project_dir / 'project.json'
    project.write_text(json.dumps(dict(schema_version=1, name='M4 native', logical_width=320, logical_height=180,
        assets=[dict(id=19,path='assets/pixel.png',width=1,height=1)],
        scene=dict(entities=[dict(id=73,asset_id=19,name='pixel',position=dict(x=40,y=28))]))),encoding='utf-8')
    connection = directory / 'connection.json'
    deadline = time.monotonic() + 120
    process = mcp = None
    observations = []
    host_id = 0

    def wait(check):
        while time.monotonic() < deadline:
            # 정상 close는 ACK 게시 직후 종료할 수 있으므로 완료 근거를 먼저 읽는다.
            result = check()
            if result:
                return result
            if process.poll() is not None:
                raise RuntimeError('native editor stopped; see editor.log')
            time.sleep(.03)
        raise TimeoutError('native behavior deadline')

    def read(path):
        try:
            return json.loads(path.read_text(encoding='utf-8'))
        except (OSError,json.JSONDecodeError):
            return None

    def cli(*args):
        result = subprocess.run([str(BIN/f'engine-cli{SUFFIX}'),*map(str,args)],capture_output=True,encoding='utf-8',timeout=10)
        value = json.loads(result.stdout)
        assert result.returncode == 0 and value['status'] == 'ok', value
        return value['data']

    def inspect():
        return cli('session','inspect','--connection-file',connection)

    def mutate(name, command, expected='applied', via_mcp=True):
        current = inspect()
        request = dict(expected_session_id=current['session_id'],expected_revision=current['revision'],request_id=name,timeout_ms=5000,command=command)
        if via_mcp:
            mcp.tool('session_mutate',request)
        else:
            path = directory/'request.json'
            path.write_text(json.dumps(request),encoding='utf-8')
            cli('session','mutate','--connection-file',connection,'--request-file',path)
        def completed():
            value = cli('session','result','--connection-file',connection,'--session-id',current['session_id'],'--request-id',name)
            return value if value['state'] not in ['queued','running'] else None
        receipt = wait(completed)
        observations.append(dict(command=command,receipt=receipt))
        assert receipt['state'] == expected, receipt
        return request, receipt

    def host(action, **fields):
        nonlocal host_id
        host_id += 1
        (directory/'host-command.json').write_text(json.dumps(dict(id=host_id,action=action,**fields)),encoding='utf-8')
        return wait(lambda: (value if value and value['id']==host_id else None) if (value:=read(directory/'host-observation.json')) else None)

    def paused_start(name):
        mutate(name+'-start',dict(type='run_start'))
        run_id=inspect()['runtime']['run_id']
        mutate(name+'-pause',dict(type='run_pause',run_id=run_id,paused=True))
        return run_id

    def screenshot(name):
        observation=host('capture')
        path=directory/f'frame-{host_id}.png'
        wait(lambda:path.exists())
        bounds=observation['runtime_viewport']
        position=inspect()['runtime']['entities'][0]['position']
        scale=bounds[2]//320
        exact=[bounds[0]+position['x']*scale,bounds[1]+position['y']*scale,scale,scale]
        pixels=assert_native_pixels(path,exact)
        return dict(name=name,file=path.name,bounds=bounds,expected_pixel_bounds=exact,pixels=pixels)

    try:
        with (directory/'editor.log').open('wb') as log, (directory/'mcp.log').open('wb') as mcp_log:
            process=subprocess.Popen([str(BIN/f'examples/native_session_smoke{SUFFIX}'),str(directory)],stdout=log,stderr=log)
            wait(lambda:connection.exists() and read(directory/'host-observation.json'))
            mcp=McpClient(connection,mcp_log,deadline)
            mcp.initialize()
            # 실제 tool 작성/연결, 검증은 revision을 바꾸지 않는다.
            request,receipt=mutate('create',dict(type='add_behavior',file='move.rhai',source=SOURCE))
            behavior_id=receipt['effect']['behavior_id']
            assert mcp.tool('session_mutate',request)==receipt
            mutate('bind',dict(type='bind_behavior',id=73,behavior_id=behavior_id),via_mcp=False)
            revision=inspect()['revision']
            _,receipt=mutate('validate',dict(type='validate_behaviors'))
            assert not receipt['effect']['diagnostics'] and inspect()['revision']==revision
            mutate('save',dict(type='save'))
            saved=project.read_bytes()
            assert json.loads(saved)['schema_version']==2
            initial=inspect()['project']['document']
            host('behavior_draft',behavior_id=behavior_id,source='move(99,0);')
            _,busy=mutate('draft-protected',dict(type='update_behavior',id=behavior_id,file='move.rhai',source='move(9,0);'),expected='rejected')
            assert busy['error']['code']=='editor_busy'
            assert inspect()['project']['document']==initial
            host('behavior_reset')
            host('save_reopen')
            assert inspect()['project']['document']==initial
            run_id=paused_start('normal')
            before=inspect()
            request,receipt=mutate('input-step',dict(type='run_step',run_id=run_id,ticks=12,input={**EMPTY,'right':True}))
            after=inspect()
            assert after['runtime']['tick']==before['runtime']['tick']+12
            expected_x=40+min(after['runtime']['tick'],40)+24
            assert after['runtime']['entities'][0]['position']==dict(x=expected_x,y=28)
            assert after['runtime']['entities'][0]['state']==after['runtime']['tick']
            assert mcp.tool('session_mutate',request)==receipt
            assert inspect()['runtime']['tick']==after['runtime']['tick']
            assert after['revision']==before['revision'] and after['project']['document']==initial
            pixels=[screenshot('input movement')]
            mutate('normal-stop',dict(type='run_stop',run_id=run_id))
            assert project.read_bytes()==saved
            # 같은 입력 없음/tick에서 editor와 배포 player의 상태·공통 픽셀 비교.
            run_id=paused_start('parity')
            mutate('parity-step',dict(type='run_step',run_id=run_id,ticks=73,input=EMPTY))
            parity=inspect()['runtime']
            pixels.append(screenshot('player parity'))
            mutate('parity-stop',dict(type='run_stop',run_id=run_id))
            player=subprocess.run([str(BIN/f'engine-player{SUFFIX}'),str(project),'--headless-ticks',str(parity['tick'])],capture_output=True,encoding='utf-8',timeout=10,check=True)
            player_status=json.loads(player.stdout)
            for field in ['tick','frame_hash','entities','logs','error']:
                assert player_status[field]==parity[field],(field,player_status,parity)
            # 문법 오류는 검증과 시작에서 진단, undo/redo 및 저장 파일은 보존.
            mutate('syntax',dict(type='update_behavior',id=behavior_id,file='move.rhai',source='// syntax\nmove(;'))
            _,syntax=mutate('syntax-validate',dict(type='validate_behaviors'))
            assert syntax['effect']['diagnostics'][0]['line']==2
            mutate('syntax-start',dict(type='run_start'),expected='rejected')
            assert inspect()['runtime'] is None
            mutate('syntax-undo',dict(type='undo'))
            assert inspect()['project']['document']==initial
            mutate('syntax-redo',dict(type='redo'))
            assert inspect()['project']['document']['behaviors'][0]['source']=='// syntax\nmove(;'
            mutate('syntax-undo-again',dict(type='undo'))
            errors=[]
            for name,source,line in [
                ('loop','if primary {\nmove(1,0);\nloop {}\n}',3),
                ('divide','if primary {\nmove(1,0);\nlet z=0;\nset_position(0,1/z);\n}',4),
                ('native','if primary {\nlet text="a";\npad(text,2,"");\n}',3),
                ('type','if primary {\nset_state(1);\nset_position(true,0);\n}',3),
            ]:
                mutate(name+'-source',dict(type='update_behavior',id=behavior_id,file='move.rhai',source=source))
                run_id=paused_start(name)
                before=inspect()
                request,receipt=mutate(name+'-step',dict(type='run_step',run_id=run_id,ticks=1,input={**EMPTY,'primary':True}),expected='rejected')
                failed=inspect()
                error=failed['runtime']['error']
                assert error['file']=='move.rhai' and error['line']==line and error['entity_id']==73,error
                assert failed['runtime']['paused'] and failed['runtime']['tick']==before['runtime']['tick']
                assert failed['runtime']['entities']==before['runtime']['entities']
                assert failed['runtime']['frame_hash']==before['runtime']['frame_hash']
                assert failed['project']==before['project'] and failed['revision']==before['revision']
                assert mcp.tool('session_mutate',request)==receipt
                mutate(name+'-resume',dict(type='run_pause',run_id=run_id,paused=False),expected='rejected')
                errors.append(error)
                mutate(name+'-stop',dict(type='run_stop',run_id=run_id))
                assert project.read_bytes()==saved
            mutate('restore-source',dict(type='update_behavior',id=behavior_id,file='move.rhai',source=SOURCE))
            mutate('final-save',dict(type='save'))
            host('save_reopen')
            assert inspect()['project']['document']==initial
            host('close')
            assert process.wait(timeout=10)==0
            # native player도 같은 행동 코드를 읽고 실제 GPU output을 만든다.
            subprocess.run([str(BIN/f'examples/native_player_smoke{SUFFIX}'),str(project),str(directory)],check=True,timeout=30)
            native_state=read(directory/'player-frame.json')
            assert native_state['runtime']['tick']>0 and native_state['runtime']['error'] is None
            assert native_state['runtime']['entities'][0]['state']==native_state['runtime']['tick']
            viewport=native_state['viewport']
            native_position=native_state['runtime']['entities'][0]['position']
            assert native_position['x']==40+min(native_state['runtime']['tick'],40)
            scale=viewport[2]//320
            player_pixels=assert_native_pixels(directory/'player-frame.png',[viewport[0]+native_position['x']*scale,viewport[1]+native_position['y']*scale,scale,scale])
            # headless error stdout에도 정확한 원인과 소스 위치가 남는다.
            bad=json.loads(project.read_text(encoding='utf-8'))
            bad['behaviors'][0]['source']='// player failure\nset_position(true,0);'
            bad_path=project_dir/'error.json'
            bad_path.write_text(json.dumps(bad),encoding='utf-8')
            result=subprocess.run([str(BIN/f'engine-player{SUFFIX}'),str(bad_path),'--headless-ticks','1'],capture_output=True,encoding='utf-8',timeout=10)
            assert result.returncode!=0
            player_error=json.loads(result.stdout)['error']
            assert player_error['line']==2 and player_error['entity_id']==73
            summary=dict(result='passed',physical_input=False,pixels=pixels,player_pixels=player_pixels,
                         parity=parity,player=player_status,native_player=native_state,errors=errors,player_error=player_error,observations=observations,
                         saved_sha256=hashlib.sha256(saved).hexdigest())
            (directory/'behavior-summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
            print('M4 native behavior author/bind/validate/CLI/MCP/editor/player smoke passed')
    finally:
        if mcp is not None:
            mcp.close()
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=5)


if __name__=='__main__':
    main()
