#[cfg(test)]
mod test {
    use crate::build;
    use parser::FileSourceSet;
    use data::{Arguments, CompilerArguments, RunnerSettings};
    use std::{env, path, fs};

    /// Tests directory
    //static TESTS: str = "../lib/test/test:";

    /// Main test
    #[test]
    pub fn test_magpie() {
        test_recursive("../../lib/test/test");
    }

    /// Recursively searches for files in the test folder to run as a test
    fn test_recursive(path: &str) {
        for entry in fs::read_dir(path).unwrap() {

            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() { // supposedly, this is a test file
                let mod_path = path.to_str().unwrap().replace(path::MAIN_SEPARATOR, "::");
                if !mod_path.ends_with(".rv") {
                    println!("File {} doesn't have the right file extension!", mod_path);
                    continue;
                }
                println!("Running {}", mod_path);
                let mod_path = format!("{}::test", &mod_path[path.parent().unwrap().to_str().unwrap().len()+2..mod_path.len() - 3]);
                let mut arguments = Arguments::build_args(
                    false,
                    RunnerSettings {
                        sources: vec![],
                        compiler_arguments: CompilerArguments {
                            compiler: "llvm".to_string(),
                            target: mod_path.clone(),
                            temp_folder: env::current_dir().unwrap().join("target"),
                        },
                    },
                );

                match build::<bool>(&mut arguments, vec![Box::new(FileSourceSet { root: path })]) {
                    Ok(inner) => match inner {
                        Some(found) => {
                            if !found {
                                assert!(false, "Failed test {}!", mod_path)
                            }
                        }
                        None => assert!(false, "Failed to find method test in test {}", mod_path),
                    },
                    Err(()) => assert!(false, "Failed to compile test {}!", mod_path),
                }
            } else if path.is_dir() { // supposedly, this is a sub-directory in the test folder
                test_recursive(path.to_str().unwrap());
            } else {
                println!("Unknown element in test folder!");
                continue;
            }
        }
    }
}
