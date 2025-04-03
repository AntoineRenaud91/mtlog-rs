use std::{collections::HashMap, fs::File, io::{Seek, SeekFrom, Write}};

use uuid::Uuid;

pub trait LogWriter {
    fn regular(&mut self, line: &str);
    fn progress(&mut self, line: &str, id: Uuid);
    fn finished(&mut self, id: Uuid);
}

fn replace_line_in_file(file:&mut File,line: &str, pos: u64) {
    file.seek(SeekFrom::Start(pos)).unwrap();
    write!(file,"{line}").unwrap();
    file.seek(SeekFrom::End(0)).unwrap();
}

pub struct LogFile {
    file: File,
    progress_positions: HashMap<Uuid,u64>
}

impl LogFile {
    pub fn new<P:AsRef<std::path::Path>>(path: P) -> Result<Self,std::io::Error> {
        let mut file = File::options().create(true).truncate(false).write(true).open(&path)?;
        file.seek(SeekFrom::End(0)).unwrap();
        Ok(Self{file,progress_positions: HashMap::new()})
    }
}

impl LogWriter for LogFile {
    fn regular(&mut self, line: &str) {
        writeln!(self.file,"{line}").unwrap()
    }

    fn progress(&mut self, line: &str, id: Uuid) {
        if let Some(pos) = self.progress_positions.get(&id) {
            replace_line_in_file(&mut self.file,line,*pos);
        } else {
            let pos = self.file.metadata().unwrap().len();
            self.progress_positions.insert(id, pos);
            writeln!(self.file,"{line}").unwrap();
        }
    }

    fn finished(&mut self, id: Uuid) {
        self.progress_positions.remove(&id);
    }
}

#[test]
fn test_log_file() {
    std::fs::remove_file("/tmp/test_log_file.log").ok();
    let mut log_file = LogFile::new("/tmp/test_log_file.log").unwrap();
    let uuid = Uuid::default();
    log_file.regular("Hello, world!");
    log_file.progress("lorem ipsum", uuid);
    log_file.regular("rust is awesome !");
    log_file.progress("LOREM IPSUM", uuid);
    log_file.finished(uuid);
    log_file.regular("test");
    assert_eq!(std::fs::read_to_string("/tmp/test_log_file.log").unwrap(),"Hello, world!\nLOREM IPSUM\nrust is awesome !\ntest\n");
}

#[derive(Default, Debug)]
pub struct LogStdout {
    progress_positions: HashMap<Uuid,usize>,
    line_counter: usize
}

impl LogWriter for LogStdout {
    fn regular(&mut self, line: &str) {
        if !self.progress_positions.is_empty(){
            self.line_counter += 1;
        }
        println!("{line}");
        std::io::stdout().flush().unwrap();
    }

    fn progress(&mut self, line: &str, id: Uuid) {
        if let Some(pos) = self.progress_positions.get(&id) {
            let pos = self.line_counter+1-pos;       
            print!("\x1B[{pos}A\r");
            print!("{line}");
            print!("\x1B[{pos}B\r");
            std::io::stdout().flush().unwrap();
        } else {
            println!("{line}");
            std::io::stdout().flush().unwrap();
            self.line_counter += 1;
            self.progress_positions.insert(id, self.line_counter);
        }
    }

    fn finished(&mut self, id: Uuid) {
        self.progress_positions.remove(&id);
        if self.progress_positions.is_empty(){
            self.line_counter = 0;
        }
    }
}


#[test]
fn test_log_stdout() {
    let mut log_stdout = LogStdout::default();
    let uuid_1 = Uuid::new_v4();
    let uuid_2 = Uuid::new_v4();
    log_stdout.regular("Hello, world!");
    log_stdout.progress("lorem ipsum", uuid_1);
    log_stdout.progress("ipsum lorem", uuid_2);
    log_stdout.regular("rust is awesome !");
    log_stdout.progress("LOREM IPSUM", uuid_2);
    log_stdout.finished(uuid_2);
    log_stdout.regular("test");
    log_stdout.progress("LOREM IPSUM", uuid_1);
    log_stdout.finished(uuid_1);
}
