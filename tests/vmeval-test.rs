use bund_blobstore::BUND;
use bund_blobstore::vm::init_adam;

const TEST1: &str = r#"
42
"#;

const TEST2: &str = r#"
"Hello world!" println
"#;

const TEST3: &str = r#"
2 40 +
"#;

const TEST4: &str = r#"
"Hello world! I am a console.typewriter" console.typewriter
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn init_adam_bund() {
        let _ = init_adam();
    }

    #[test]
    fn test_eval_1() {
        init_adam_bund();
        let mut vm = BUND.get().unwrap().write();
        let _ = vm.vm.stack.clear();
        let vm = vm.eval(TEST1).unwrap();
        assert_eq!(vm.vm.stack.current_stack_len(), 1);
        let data = vm.vm.stack.pull().unwrap();
        assert_eq!(data.cast_int().unwrap() as i64, 42 as i64);
    }

    #[test]
    fn test_eval_2() {
        init_adam_bund();
        let mut vm = BUND.get().unwrap().write();
        let _ = vm.vm.stack.clear();
        let _ = vm.eval(TEST2).unwrap();
    }

    #[test]
    fn test_eval_3() {
        init_adam_bund();
        let mut vm = BUND.get().unwrap().write();
        let _ = vm.vm.stack.clear();
        let vm = vm.eval(TEST3).unwrap();
        assert_eq!(vm.vm.stack.current_stack_len(), 1);
        let data = vm.vm.stack.pull().unwrap();
        assert_eq!(data.cast_int().unwrap() as i64, 42 as i64);
    }

    #[test]
    fn test_eval_4() {
        init_adam_bund();
        let mut vm = BUND.get().unwrap().write();
        let _ = vm.vm.stack.clear();
        let _ = vm.eval(TEST4).unwrap();
    }
}
