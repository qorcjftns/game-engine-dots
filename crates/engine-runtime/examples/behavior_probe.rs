//! M4 선행 실험: 한정된 텍스트 명령을 파싱하고 실행 예산으로 중단한다.
//! 제품 행동 언어 선택이나 프로젝트 스크립트 지원을 의미하지 않는다.
#[derive(Debug, Clone, Copy)]
enum Instruction {
    Add(i64),
    Jump(usize),
    Fail,
}
fn compile(source: &str) -> Result<Vec<Instruction>, String> {
    source
        .lines()
        .enumerate()
        .map(|(line, text)| {
            let parts: Vec<_> = text.split_whitespace().collect();
            match parts.as_slice() {
                ["add", value] => value
                    .parse()
                    .map(Instruction::Add)
                    .map_err(|_| format!("{}: 정수 오류", line + 1)),
                ["jump", value] => value
                    .parse()
                    .map(Instruction::Jump)
                    .map_err(|_| format!("{}: 위치 오류", line + 1)),
                ["fail"] => Ok(Instruction::Fail),
                _ => Err(format!("{}: 명령 오류", line + 1)),
            }
        })
        .collect()
}
fn execute(code: &[Instruction], budget: u32) -> Result<i64, String> {
    let (mut pc, mut value) = (0usize, 0i64);
    for _ in 0..budget {
        if pc == code.len() {
            return Ok(value);
        }
        match code
            .get(pc)
            .ok_or_else(|| format!("{}: 위치 범위 오류", pc + 1))?
        {
            Instruction::Add(amount) => {
                value = value.checked_add(*amount).ok_or("정수 범위 오류")?;
                pc += 1;
            }
            Instruction::Jump(target) => pc = *target,
            Instruction::Fail => return Err(format!("{}: 명시적 실행 오류", pc + 1)),
        }
    }
    if pc == code.len() {
        Ok(value)
    } else {
        Err(format!("{}: 실행 예산 초과", pc + 1))
    }
}
fn main() {
    assert_eq!(execute(&compile("add 3\nadd 4").unwrap(), 100).unwrap(), 7);
    assert_eq!(execute(&compile("add 3\nadd 4").unwrap(), 2).unwrap(), 7);
    assert!(compile("invalid").unwrap_err().contains("1:"));
    assert!(
        execute(&compile("jump 0").unwrap(), 10000)
            .unwrap_err()
            .contains("예산")
    );
    assert!(
        execute(&compile("fail").unwrap(), 100)
            .unwrap_err()
            .contains("1:")
    );
    println!("정상 규칙/문법 오류/무한 반복 예산 중단/실행 오류 4개 확인; 호스트 IO 없음");
}
