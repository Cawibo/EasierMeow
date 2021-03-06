extern crate reqwest;
extern crate select;

mod app {

    use std::env;
    use std::error::Error;

    pub fn run_app() -> Result<(), Box<dyn Error>> {
        let mut args: Vec<String> = env::args().collect();

        if args.len() == 1 {
            println!("Please use a command.",);
            return Ok(());
        }

        let meow_args = args.split_off(2);

        match args[1].as_str() {
            "init" => init::init(meow_args),
            "test" => test::test(),
            _ => println!("Valid commands include: init",),
        }

        Ok(())
    }

    mod test {
        use std::fs;
        use std::process::Command;

        fn run_test(n: i8) -> bool {
            let test_name = format!("test{}.in", n);
            let arg = format!("Python main.py < {}", test_name);
            let output = Command::new("cmd")
                .args(&["/C", arg.as_str()])
                .output()
                .expect(&format!("failed to run test {}", test_name));

            let res: String = String::from_utf8_lossy(&output.stdout).to_string();
            let err: String = String::from_utf8_lossy(&output.stderr).to_string();
            if !output.status.success() {
                println!("Test {} failed!\n{}", n, err);
                false
            } else {
                let file_name = format!("test{}.out", n);
                let theirs = fs::read_to_string(file_name).unwrap_or("".to_string());

                if theirs.trim() == res.trim() {
                    true
                } else {
                    false
                }
            }
        }

        pub fn test() {
            let mut counter = 0;
            while let Ok(_) = fs::read_to_string(format!("test{}.in", counter)) {
                match run_test(counter) {
                    true => println!("Testcase {} succeeded.", counter),
                    false => println!("Testcase {} failed.", counter),
                };

                counter += 1;
            }
        }
    }

    mod init {
        use reqwest::StatusCode;
        use select::document::Document;
        use select::predicate::{And, Any, Child, Class, Element, Name};
        use std::fs;
        use std::io::Write;

        #[derive(Default)]
        struct KattisData {
            description: String,
            input_description: String,
            output_description: String,
            tests: std::vec::Vec<(String, String)>,
        }

        fn prompt(prompt: String) -> String {
            print!("{}: ", prompt);
            std::io::stdout().flush().unwrap();

            let mut input = String::new();
            match std::io::stdin().read_line(&mut input) {
                Ok(_n) => {
                    // println!("{} bytes read", n);
                    // println!("{}", input);
                    input.trim().to_string()
                }
                Err(e) => {
                    println!("error: {}", e);
                    "".to_string()
                }
            }
        }

        fn make_header(data: &KattisData) -> String {
            let input = &data.input_description;
            let output = &data.output_description;

            format!(
                "#Input: {input}\n\n#Output: {output}\n\n",
                input = input,
                output = output
            )
        }

        fn write_main_file(content: String, language: String) -> Result<(), std::io::Error> {
            fn file_ending(language: String) -> String {
                match language.to_lowercase().as_ref() {
                    "python" => ".py".to_string(),
                    _ => "".to_string(),
                }
            }
            let file_ending: String = file_ending(language);
            let file_name: String = format!("main{}", file_ending);
            fs::write(file_name, content)
        }

        fn write_test_files(tests: &std::vec::Vec<(String, String)>) -> Result<(), std::io::Error> {
            for (i, (input, output)) in tests.iter().enumerate() {
                fs::write(format!("test{}.in", i), input)?;
                fs::write(format!("test{}.out", i), output)?;
            }

            Ok(())
        }

        use url::Url;

        #[derive(Debug)]
        struct Args {
            url: String,
            language: String,
        }

        fn handle_url_argument(arg: String) -> String {
            match Url::parse(arg.as_str()) {
                Ok(_) => {
                    // println!("accepted as url: {}", &arg);
                    arg.to_string()
                }
                Err(_) => format!("https://open.kattis.com/problems/{}", arg),
            }
        }

        fn handle_arguments(args: Vec<String>) -> Args {
            let url: String = if args.len() < 1 {
                let tmp = prompt("Problem: ".to_string());
                handle_url_argument(tmp)
            } else {
                handle_url_argument(args[0].to_string())
            };

            // let language: String = if args.len() < 2 {
            //     prompt("Language: ".to_string())
            // } else {
            //     args[1].to_string()
            // };
            let language = "Python".to_string();
            Args { url, language }
        }

        pub fn init(args: Vec<String>) {
            let h_args: Args = handle_arguments(args);

            let initialization = || -> Result<(), std::io::Error> {
                let document = fetch_document(h_args.url).unwrap();
                let data = parse_document(document).unwrap();
                let header: String = make_header(&data);
                write_main_file(header, h_args.language)?;
                write_test_files(&data.tests)?;
                Ok(())
            };

            if let Err(_err) = initialization() {
                panic!("Failed to initiate Kattis files.",);
            }
        }

        fn fetch_document(url: String) -> Result<Document, Box<dyn std::error::Error>> {
            let resp = reqwest::blocking::get(&url)?;

            match resp.status() {
                StatusCode::OK => Ok(Document::from_read(resp).unwrap()),
                _ => panic!("Kattis is unavailable at the moment."), // TODO: return and handle error instead of killing process.
            }
        }

        fn parse_document(document: Document) -> Result<KattisData, std::io::Error> {
            let mut content_check: i8 = 0;
            let mut parsed_data = KattisData::default();

            for node in document.find(And(Element, Child(Class("problembody"), Any))) {
                match node.name() {
                    Some("p") => {
                        let text = node.text().trim().replace("\n   ", "").replace("$", "");
                        match content_check {
                            0 => parsed_data.description.push_str(&text),
                            1 => parsed_data.input_description.push_str(&text),
                            2 => parsed_data.output_description.push_str(&text),
                            _ => panic!("Encountered trouble parsing data: content_check > 2."), // TODO: make this an error.
                        }
                    }
                    Some("h2") => content_check += 1,
                    Some("table") => {
                        parsed_data.tests.push(parse_table(node));
                    }
                    Some(&_) | None => continue,
                };
            }
            Ok(parsed_data)
        }

        fn parse_table(table: select::node::Node) -> (String, String) {
            let res: Vec<String> = table
                .find(And(Element, Name("pre")))
                .map(|n| n.inner_html())
                .collect();

            (res[0].to_string(), res[1].to_string())
        }
    }
}

fn main() {
    std::process::exit(match app::run_app() {
        Ok(_) => 0,
        Err(err) => {
            eprintln!("error: {:?}", err);
            1
        }
    });
}
