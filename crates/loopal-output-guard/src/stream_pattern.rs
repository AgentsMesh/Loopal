use secrecy::{ExposeSecret, SecretString};
use zeroize::Zeroizing;

pub(super) struct Pattern {
    pub(super) name: String,
    pub(super) value: SecretString,
    prefix: Zeroizing<Vec<usize>>,
    state: usize,
}

impl Pattern {
    pub(super) fn new(name: String, value: SecretString) -> Self {
        let bytes = value.expose_secret().as_bytes();
        let mut prefix = Zeroizing::new(vec![0; bytes.len()]);
        let mut matched = 0;
        for index in 1..bytes.len() {
            while matched > 0 && bytes[index] != bytes[matched] {
                matched = prefix[matched - 1];
            }
            if bytes[index] == bytes[matched] {
                matched += 1;
            }
            prefix[index] = matched;
        }
        Self {
            name,
            value,
            prefix,
            state: 0,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.value.expose_secret().len()
    }

    pub(super) fn state(&self) -> usize {
        self.state
    }

    pub(super) fn prune(&mut self, max_state: usize) {
        while self.state > max_state {
            self.state = self.prefix[self.state - 1];
        }
    }

    pub(super) fn advance(&mut self, byte: u8) -> bool {
        let bytes = self.value.expose_secret().as_bytes();
        while self.state > 0 && byte != bytes[self.state] {
            self.state = self.prefix[self.state - 1];
        }
        if byte == bytes[self.state] {
            self.state += 1;
        }
        if self.state != bytes.len() {
            return false;
        }
        self.state = self.prefix[bytes.len() - 1];
        true
    }
}
