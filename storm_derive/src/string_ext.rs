pub(crate) trait StringExt {
    fn add(&mut self, ch: char) -> &mut Self;

    fn add_str(&mut self, s: &str) -> &mut Self;

    fn add_sep(&mut self, sep: char) -> &mut Self;

    fn add_sep_str(&mut self, sep: &str) -> &mut Self;
}

impl StringExt for String {
    fn add(&mut self, ch: char) -> &mut Self {
        self.push(ch);
        self
    }

    fn add_str(&mut self, s: &str) -> &mut Self {
        self.push_str(s);
        self
    }

    fn add_sep(&mut self, sep: char) -> &mut Self {
        if !self.is_empty() {
            self.push(sep);
        }
        self
    }

    fn add_sep_str(&mut self, sep: &str) -> &mut Self {
        if !self.is_empty() {
            self.push_str(sep)
        }
        self
    }
}
