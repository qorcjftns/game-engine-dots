"""실제 editor/player 창과 CLI/MCP를 교차 실행하는 M3 회귀. physical input과 구분한다."""
import hashlib
import json
import subprocess
import time
import struct
import zlib
from pathlib import Path
from native_session_smoke import McpClient, SUFFIX
from smoke_readonly import png_pixel

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / 'target/debug'

def assert_native_pixels(path, bounds=None):
    """스크린샷 PNG를 표준 라이브러리로 복원해 검정/빈 프레임을 거부한다."""
    data=path.read_bytes()
    assert data[:8]==b"\x89PNG\r\n\x1a\n", path
    offset=8; compressed=bytearray(); width=height=channels=0
    while offset<len(data):
        length=struct.unpack(">I",data[offset:offset+4])[0]
        kind=data[offset+4:offset+8]; value=data[offset+8:offset+8+length]
        if kind==b'IHDR':
            width,height,depth,color,_,_,interlace=struct.unpack('>IIBBBBB',value)
            assert depth==8 and color in [2,6] and interlace==0
            channels=4 if color==6 else 3
        if kind==b'IDAT': compressed.extend(value)
        offset+=length+12
    assert width>=320 and height>=180
    x0,y0,w,h=bounds or (0,0,width,height)
    assert w>0 and h>0 and x0+w<=width and y0+h<=height
    raw=zlib.decompress(compressed); stride=width*channels; previous=bytearray(stride); magenta=0
    assert len(raw)==height*(stride+1)
    for y in range(height):
        start=y*(stride+1); kind=raw[start]; row=bytearray(raw[start+1:start+1+stride])
        for x in range(stride):
            left=row[x-channels] if x>=channels else 0
            up=previous[x]; corner=previous[x-channels] if x>=channels else 0
            if kind==1: predictor=left
            elif kind==2: predictor=up
            elif kind==3: predictor=(left+up)//2
            elif kind==4:
                estimate=left+up-corner; a,b,c=abs(estimate-left),abs(estimate-up),abs(estimate-corner)
                predictor=left if a<=b and a<=c else (up if b<=c else corner)
            else: assert kind==0; predictor=0
            row[x]=(row[x]+predictor)&255
        if y0<=y<y0+h:
            magenta+=sum(row[x]>240 and row[x+1]<20 and row[x+2]>240 for x in range(x0*channels,(x0+w)*channels,channels))
        previous=row
    assert magenta>0, f"native fixture pixels missing: {path}"
    return {'width':width,'height':height,'magenta_pixels':magenta}

def main():
    directory = ROOT / 'artifacts/native-runtime'
    directory.mkdir(parents=True, exist_ok=False)
    project_dir = directory / 'project'
    (project_dir / 'assets').mkdir(parents=True)
    (project_dir / 'assets/pixel.png').write_bytes(png_pixel())
    project = project_dir / 'project.json'
    project.write_text(json.dumps({'schema_version':1,'name':'M3 native','logical_width':320,'logical_height':180,
        'assets':[{'id':19,'path':'assets/pixel.png','width':1,'height':1}],
        'scene':{'entities':[{'id':73,'asset_id':19,'name':'pixel','position':{'x':40,'y':28}}]}}),encoding='utf-8')
    original = hashlib.sha256(project.read_bytes()).hexdigest()
    connection = directory / 'connection.json'
    deadline = time.monotonic() + 90
    process = None
    mcp = None
    observations = []
    def wait(check):
        while time.monotonic() < deadline:
            if process.poll() is not None: raise RuntimeError('native editor stopped')
            result = check()
            if result: return result
            time.sleep(.03)
        raise TimeoutError('native runtime deadline')
    def read(path):
        try: return json.loads(path.read_text(encoding='utf-8'))
        except (OSError,json.JSONDecodeError): return None
    def cli(*args):
        result = subprocess.run([str(BIN/f'engine-cli{SUFFIX}'),*map(str,args)],capture_output=True,encoding='utf-8',timeout=10)
        value = json.loads(result.stdout)
        assert result.returncode == 0 and value['status'] == 'ok', value
        return value['data']
    def inspect(): return cli('session','inspect','--connection-file',connection)
    def mutate(name, command, via_mcp=False):
        current=inspect()
        request={'expected_session_id':current['session_id'],'expected_revision':current['revision'],'request_id':name,'timeout_ms':5000,'command':command}
        if via_mcp: mcp.tool('session_mutate',request)
        else:
            path=directory/'request.json'; path.write_text(json.dumps(request),encoding='utf-8')
            cli('session','mutate','--connection-file',connection,'--request-file',path)
        def completed():
            value=cli('session','result','--connection-file',connection,'--session-id',current['session_id'],'--request-id',name)
            return value if value['state'] not in ['queued','running'] else None
        receipt=wait(completed);observations.append({'command':command,'receipt':receipt});return request,receipt
    try:
        with (directory/'editor.log').open('wb') as log, (directory/'mcp.log').open('wb') as mcp_log:
            process=subprocess.Popen([str(BIN/f'examples/native_session_smoke{SUFFIX}'),str(directory)],stdout=log,stderr=log)
            wait(lambda: connection.exists() and read(directory/'host-observation.json'))
            mcp=McpClient(connection,mcp_log,deadline);mcp.initialize()
            first=inspect()
            _,receipt=mutate('start',{'type':'run_start'});assert receipt['state']=='applied',receipt
            run_id=inspect()['runtime']['run_id']
            _,receipt=mutate('pause',{'type':'run_pause','run_id':run_id,'paused':True},True);assert receipt['state']=='applied',receipt
            before=inspect()['runtime']['tick']
            action={'left':True,'right':False,'up':False,'down':False,'primary':False,'pointer':None}
            request,receipt=mutate('step',{'type':'run_step','run_id':run_id,'ticks':73,'input':action},True);assert receipt['state']=='applied',receipt
            assert mcp.tool('session_mutate',request)==receipt
            after=inspect();assert after['runtime']['tick']==before+73,after
            assert after['runtime']['input']==action
            assert after['project']==first['project'] and after['revision']==first['revision']
            _,blocked=mutate('edit-during-play',{'type':'undo'});assert blocked['state']=='rejected' and blocked['error']['code']=='editor_busy',blocked
            (directory/'host-command.json').write_text(json.dumps({'id':1,'action':'capture'}),encoding='utf-8')
            wait(lambda:(directory/'frame-1.png').exists())
            viewport=wait(lambda:(read(directory/'host-observation.json') or {}).get('runtime_viewport'))
            assert viewport is not None
            assert inspect()['runtime']['frame']>0
            _,receipt=mutate('stop',{'type':'run_stop','run_id':run_id});assert receipt['state']=='applied'
            _,receipt=mutate('restart',{'type':'run_start'});assert receipt['state']=='applied'
            next_id=inspect()['runtime']['run_id'];assert next_id!=run_id
            _,stale=mutate('stale',{'type':'run_stop','run_id':run_id},True);assert stale['state']=='rejected' and stale['error']['code']=='run_mismatch',stale
            _,stopped=mutate('stop-new',{'type':'run_stop','run_id':next_id})
            assert stopped['state']=='applied',stopped
            _,edited=mutate('edit-after-stop',{'type':'move_entity','id':73,'position':{'x':57,'y':32}})
            assert edited['state']=='applied' and inspect()['is_dirty'],edited
            assert inspect()['project']['document']['scene']['entities'][0]['position']=={'x':57,'y':32}
            _,undone=mutate('undo-after-stop',{'type':'undo'})
            assert undone['state']=='applied' and not inspect()['is_dirty'],undone
            assert inspect()['project']==first['project']
            assert hashlib.sha256(project.read_bytes()).hexdigest()==original
            (directory/'host-command.json').write_text(json.dumps({'id':2,'action':'close'}),encoding='utf-8')
            assert process.wait(timeout=10)==0
            player=subprocess.run([str(BIN/f'engine-player{SUFFIX}'),str(project),'--headless-ticks','73'],capture_output=True,encoding='utf-8',timeout=10,check=True)
            player_state=json.loads(player.stdout)
            assert player_state['tick']==73 and player_state['frame_hash']==after['runtime']['frame_hash']
            subprocess.run([str(BIN/f'examples/native_player_smoke{SUFFIX}'),str(project),str(directory)],check=True,timeout=30)
            assert (directory/'player-frame.png').exists()
            pixels={'editor_viewport':assert_native_pixels(directory/'frame-1.png',viewport),'player':assert_native_pixels(directory/'player-frame.png')}
            summary={'pixels':pixels,'result':'passed','physical_input':False,'exact_ticks':73,'editor':after['runtime'],'player':player_state,'observations':observations}
            (directory/'runtime-summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
            print('M3 native editor/CLI/MCP/player smoke passed')
    finally:
        if mcp is not None: mcp.close()
        if process is not None and process.poll() is None: process.terminate();process.wait(timeout=5)

if __name__=='__main__': main()
