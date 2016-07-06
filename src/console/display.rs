use std::ffi::*;
use pdcurses::*;

use nbplink::nbp::routing;

pub struct Display {
    window: *mut WINDOW,
    messages: Vec<CString>,
    input: String
}

pub fn new() -> Display {
    unsafe {
        let window = initscr();
        nodelay(window, 1);

        wmove(window, getmaxy(window)-1, 0);
        wrefresh(window);

        Display {
            window: window,
            messages: vec!(),
            input: String::new()
        }
    }
}

impl Display {
    pub fn get_input(&mut self) -> Option<String> {
        unsafe {
            {
                //Read in all available characters
                let mut chr = wgetch(self.window);

                while chr != -1 {
                    self.input.push(chr as u8 as char);
                    chr = wgetch(self.window);
                }
            }

            //Next translate new lines
            self.input.replace("\r\n", "\n");

            //Parse any input
            match self.input.find('\n') {
                Some(idx) => {
                    let result = self.input.split_at(idx+1).0.to_string();
                    self.input = self.input[idx+1..].to_string();

                    wclear(self.window);

                    Some(result)
                },
                None => None
            }

        }
    }

    pub fn push_message(&mut self, msg: &String) {
        self.messages.push(CString::new(msg.as_str()).unwrap());

        unsafe {
            wclear(self.window);
            for (i,msg) in self.messages.iter().rev().enumerate() {
                wmove(self.window, getmaxy(self.window) - (i as i32 + 2), 0);
                waddstr(self.window, msg.as_ptr());
            }

            wmove(self.window, getmaxy(self.window) - 1, 0);
            refresh();
        }
    }

    pub fn exit() {
        unsafe {
            endwin();
        }
    }
}