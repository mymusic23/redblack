use crossterm::event::{read, Event, KeyCode, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use redgold_schema::{ErrorInfoContext, RgResult};
use redgold_schema::conf::node_config::NodeConfig;
use redgold_schema::keys::words_pass::WordsPass;

pub fn get_input(prompt: impl Into<String>, is_password: bool) -> RgResult<Option<String>> {
    println!("{}", prompt.into());

    if !is_password {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).error_info("Failed to read line")?;
        return if input.is_empty() { Ok(None) } else { Ok(Some(input)) };
    }

    // Password input handling
    enable_raw_mode().error_info("Failed to enable raw mode")?;
    let mut password = String::new();

    loop {
        match read().error_info("Failed to read event")? {
            Event::Key(KeyEvent { code, .. }) => match code {
                KeyCode::Enter => break,
                KeyCode::Backspace => { password.pop(); },
                KeyCode::Char(c) => {
                    password.push(c);
                    print!("*");
                },
                _ => {}
            },
            _ => {}
        }
    }

    disable_raw_mode().error_info("Failed to disable raw mode")?;
    println!(); // New line after password entry

    Ok(Some(password))
}

pub trait NodeConfigExt {
    fn cli_get_words_pass(&self, default_behavior: bool) -> WordsPass;
}

impl NodeConfigExt for NodeConfig {
    fn cli_get_words_pass(&self, default_behavior: bool) -> WordsPass {
        // TODO move to nodeconfig impl
        let mut pass = self.config_data.cli.as_ref().and_then(|c| c.passphrase.clone()).unwrap_or(default_behavior);
        if self.config_data.cli.as_ref().and_then(|c| c.non_interactive.clone()).unwrap_or(false) {
            pass = false;
        }

        let pass = if pass {
            get_input("Enter passphrase: ", true)
                .unwrap()
        } else {
            None
        };

        let mut default = self.secure_mnemonic_words_or();
        //
        // if let Some(w) = self.config_data.debug.as_ref().and_then(|x| x.words.clone()) {
        //     if !self.network.is_main() {
        //         default = w;
        //     }
        // }

        WordsPass::new(default, pass)
    }

}
