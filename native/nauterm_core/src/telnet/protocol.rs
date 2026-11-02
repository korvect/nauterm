use encoding_rs::Encoding;

use crate::terminal::{TerminalGeometry, TerminalOptions};

use super::codec::{encode_client_input, encode_client_input_binary};

pub(super) const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;

const OPT_BINARY: u8 = 0;
const OPT_ECHO: u8 = 1;
const OPT_SUPPRESS_GO_AHEAD: u8 = 3;
const OPT_TERMINAL_TYPE: u8 = 24;
const OPT_NAWS: u8 = 31;
const OPT_ENVIRON: u8 = 36;
const OPT_NEW_ENVIRON: u8 = 39;

const TTYPE_IS: u8 = 0;
const TTYPE_SEND: u8 = 1;
const ENV_IS: u8 = 0;
const ENV_SEND: u8 = 1;
const ENV_VAR: u8 = 0;
const ENV_VALUE: u8 = 1;
const ENV_ESCAPE: u8 = 2;
const ENV_USERVAR: u8 = 3;

pub(super) struct TelnetProtocol {
    geometry: TerminalGeometry,
    options: TerminalOptions,
    state: TelnetState,
    local_options: [NegotiationState; 256],
    remote_options: [NegotiationState; 256],
    pending_nvt_cr: bool,
}

pub(super) struct TelnetResponse {
    pub(super) output: Vec<u8>,
    pub(super) commands: Vec<u8>,
}

enum TelnetState {
    Data,
    Iac,
    Negotiation(u8),
    Subnegotiation {
        option: Option<u8>,
        data: Vec<u8>,
        escaped: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NegotiationState {
    No,
    WantYes,
    Yes,
}

impl TelnetProtocol {
    pub(super) fn new(geometry: TerminalGeometry, options: TerminalOptions) -> Self {
        Self {
            geometry,
            options,
            state: TelnetState::Data,
            local_options: [NegotiationState::No; 256],
            remote_options: [NegotiationState::No; 256],
            pending_nvt_cr: false,
        }
    }

    pub(super) fn initial_commands(&mut self) -> Vec<u8> {
        let mut commands = Vec::new();
        self.request_local_option(OPT_TERMINAL_TYPE, &mut commands);
        self.request_local_option(OPT_NAWS, &mut commands);
        self.request_local_option(OPT_NEW_ENVIRON, &mut commands);
        self.request_local_option(OPT_SUPPRESS_GO_AHEAD, &mut commands);
        self.request_remote_option(OPT_ECHO, &mut commands);
        self.request_remote_option(OPT_SUPPRESS_GO_AHEAD, &mut commands);
        commands
    }

    pub(super) fn encode_input(&self, bytes: &[u8], encoding: &'static Encoding) -> Vec<u8> {
        if self.local_option_enabled(OPT_BINARY) {
            encode_client_input_binary(bytes, encoding)
        } else {
            encode_client_input(bytes, encoding)
        }
    }

    pub(super) fn receive(&mut self, bytes: &[u8]) -> TelnetResponse {
        let mut response = TelnetResponse {
            output: Vec::with_capacity(bytes.len()),
            commands: Vec::new(),
        };

        for byte in bytes {
            self.receive_byte(*byte, &mut response);
        }

        response
    }

    pub(super) fn resize(&mut self, geometry: TerminalGeometry) -> Vec<u8> {
        self.geometry = geometry;
        if self.local_option_enabled(OPT_NAWS) {
            self.naws()
        } else {
            Vec::new()
        }
    }

    fn receive_byte(&mut self, byte: u8, response: &mut TelnetResponse) {
        let state = std::mem::replace(&mut self.state, TelnetState::Data);
        match state {
            TelnetState::Data => {
                if byte == IAC {
                    self.state = TelnetState::Iac;
                } else {
                    self.receive_data_byte(byte, response);
                }
            }
            TelnetState::Iac => match byte {
                IAC => {
                    response.output.push(IAC);
                }
                DO | DONT | WILL | WONT => {
                    self.state = TelnetState::Negotiation(byte);
                }
                SB => {
                    self.state = TelnetState::Subnegotiation {
                        option: None,
                        data: Vec::new(),
                        escaped: false,
                    };
                }
                _ => {
                    self.state = TelnetState::Data;
                }
            },
            TelnetState::Negotiation(command) => {
                response
                    .commands
                    .extend(self.handle_negotiation(command, byte));
            }
            TelnetState::Subnegotiation {
                mut option,
                mut data,
                mut escaped,
            } => {
                if option.is_none() {
                    option = Some(byte);
                    self.state = TelnetState::Subnegotiation {
                        option,
                        data,
                        escaped,
                    };
                    return;
                }
                if escaped {
                    if byte == SE {
                        let option = option.unwrap_or_default();
                        response
                            .commands
                            .extend(self.handle_subnegotiation(option, &data));
                        self.state = TelnetState::Data;
                    } else {
                        if byte == IAC {
                            data.push(IAC);
                        }
                        escaped = false;
                        self.state = TelnetState::Subnegotiation {
                            option,
                            data,
                            escaped,
                        };
                    }
                    return;
                }
                if byte == IAC {
                    escaped = true;
                } else {
                    data.push(byte);
                }
                self.state = TelnetState::Subnegotiation {
                    option,
                    data,
                    escaped,
                };
            }
        }
    }

    fn handle_negotiation(&mut self, command: u8, option: u8) -> Vec<u8> {
        match command {
            DO => {
                if self.supports_local_option(option) {
                    let previous = self.local_options[option as usize];
                    self.local_options[option as usize] = NegotiationState::Yes;
                    let mut commands = Vec::new();
                    if previous == NegotiationState::No {
                        commands.extend_from_slice(&negotiate(WILL, option));
                    }
                    if previous != NegotiationState::Yes && option == OPT_NAWS {
                        commands.extend_from_slice(&self.naws());
                    }
                    commands
                } else {
                    self.local_options[option as usize] = NegotiationState::No;
                    negotiate(WONT, option)
                }
            }
            DONT => {
                let previous = self.local_options[option as usize];
                self.local_options[option as usize] = NegotiationState::No;
                if previous == NegotiationState::Yes {
                    negotiate(WONT, option)
                } else {
                    Vec::new()
                }
            }
            WILL => {
                if self.supports_remote_option(option) {
                    let previous = self.remote_options[option as usize];
                    self.remote_options[option as usize] = NegotiationState::Yes;
                    if previous == NegotiationState::No {
                        negotiate(DO, option)
                    } else {
                        Vec::new()
                    }
                } else {
                    self.remote_options[option as usize] = NegotiationState::No;
                    negotiate(DONT, option)
                }
            }
            WONT => {
                let previous = self.remote_options[option as usize];
                self.remote_options[option as usize] = NegotiationState::No;
                if previous == NegotiationState::Yes {
                    negotiate(DONT, option)
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    fn request_local_option(&mut self, option: u8, commands: &mut Vec<u8>) {
        if self.local_options[option as usize] == NegotiationState::No {
            self.local_options[option as usize] = NegotiationState::WantYes;
            commands.extend_from_slice(&negotiate(WILL, option));
        }
    }

    fn request_remote_option(&mut self, option: u8, commands: &mut Vec<u8>) {
        if self.remote_options[option as usize] == NegotiationState::No {
            self.remote_options[option as usize] = NegotiationState::WantYes;
            commands.extend_from_slice(&negotiate(DO, option));
        }
    }

    fn local_option_enabled(&self, option: u8) -> bool {
        self.local_options[option as usize] == NegotiationState::Yes
    }

    fn remote_option_enabled(&self, option: u8) -> bool {
        self.remote_options[option as usize] == NegotiationState::Yes
    }

    fn receive_data_byte(&mut self, byte: u8, response: &mut TelnetResponse) {
        if self.remote_option_enabled(OPT_BINARY) {
            self.pending_nvt_cr = false;
            response.output.push(byte);
            return;
        }

        if self.pending_nvt_cr && byte == 0 {
            self.pending_nvt_cr = false;
            return;
        }

        if byte == b'\n' && !self.pending_nvt_cr {
            response.output.push(b'\r');
        }
        self.pending_nvt_cr = byte == b'\r';
        response.output.push(byte);
    }

    fn supports_local_option(&self, option: u8) -> bool {
        matches!(
            option,
            OPT_BINARY
                | OPT_SUPPRESS_GO_AHEAD
                | OPT_TERMINAL_TYPE
                | OPT_NAWS
                | OPT_ENVIRON
                | OPT_NEW_ENVIRON
        )
    }

    fn supports_remote_option(&self, option: u8) -> bool {
        matches!(option, OPT_BINARY | OPT_ECHO | OPT_SUPPRESS_GO_AHEAD)
    }

    fn handle_subnegotiation(&self, option: u8, data: &[u8]) -> Vec<u8> {
        match option {
            OPT_TERMINAL_TYPE if data.first() == Some(&TTYPE_SEND) => self.terminal_type(),
            OPT_ENVIRON | OPT_NEW_ENVIRON if data.first() == Some(&ENV_SEND) => {
                self.environment(option)
            }
            _ => Vec::new(),
        }
    }

    fn terminal_type(&self) -> Vec<u8> {
        let mut commands = vec![IAC, SB, OPT_TERMINAL_TYPE, TTYPE_IS];
        commands.extend_from_slice(self.options.terminal_type.term().as_bytes());
        commands.extend_from_slice(&[IAC, SE]);
        commands
    }

    fn environment(&self, option: u8) -> Vec<u8> {
        let mut commands = vec![IAC, SB, option, ENV_IS];
        for entry in &self.options.environment {
            let variable = entry.variable.trim();
            if variable.is_empty() || variable.contains('=') {
                continue;
            }
            append_environment_value(&mut commands, ENV_VAR, variable.as_bytes());
            append_environment_value(&mut commands, ENV_VALUE, entry.value.as_bytes());
        }
        commands.extend_from_slice(&[IAC, SE]);
        commands
    }

    fn naws(&self) -> Vec<u8> {
        let columns = self.geometry.columns.min(u16::MAX as usize) as u16;
        let rows = self.geometry.rows.min(u16::MAX as usize) as u16;
        let mut payload = Vec::with_capacity(9);
        payload.extend_from_slice(&[IAC, SB, OPT_NAWS]);
        append_naws_u16(&mut payload, columns);
        append_naws_u16(&mut payload, rows);
        payload.extend_from_slice(&[IAC, SE]);
        payload
    }
}

fn negotiate(command: u8, option: u8) -> Vec<u8> {
    vec![IAC, command, option]
}

fn append_naws_u16(payload: &mut Vec<u8>, value: u16) {
    append_naws_byte(payload, (value >> 8) as u8);
    append_naws_byte(payload, (value & 0xff) as u8);
}

fn append_naws_byte(payload: &mut Vec<u8>, byte: u8) {
    payload.push(byte);
    if byte == IAC {
        payload.push(IAC);
    }
}

fn append_environment_value(commands: &mut Vec<u8>, marker: u8, bytes: &[u8]) {
    commands.push(marker);
    for byte in bytes {
        match *byte {
            IAC => commands.extend_from_slice(&[IAC, IAC]),
            ENV_VAR | ENV_VALUE | ENV_ESCAPE | ENV_USERVAR => {
                commands.extend_from_slice(&[ENV_ESCAPE, *byte])
            }
            _ => commands.push(*byte),
        }
    }
}

#[cfg(test)]
mod tests {
    use encoding_rs::UTF_8;

    use super::*;
    use crate::terminal::{ColorTerm, TerminalType};

    fn count_sequence(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .filter(|window| *window == needle)
            .count()
    }

    #[test]
    fn strips_telnet_negotiation_and_replies() {
        let options = TerminalOptions {
            terminal_type: TerminalType::Xterm256Color,
            color_term: ColorTerm::Truecolor,
            ..TerminalOptions::default()
        };
        let mut protocol = TelnetProtocol::new(TerminalGeometry::new(80, 24), options);
        let response = protocol.receive(&[
            b'h',
            b'i',
            IAC,
            DO,
            OPT_TERMINAL_TYPE,
            IAC,
            WILL,
            OPT_ECHO,
            b'!',
        ]);

        assert_eq!(response.output, b"hi!");
        assert!(response
            .commands
            .windows(3)
            .any(|window| window == [IAC, WILL, OPT_TERMINAL_TYPE]));
        assert!(response
            .commands
            .windows(3)
            .any(|window| window == [IAC, DO, OPT_ECHO]));
    }

    #[test]
    fn replies_to_terminal_type_subnegotiation() {
        let options = TerminalOptions {
            terminal_type: TerminalType::Xterm,
            ..TerminalOptions::default()
        };
        let mut protocol = TelnetProtocol::new(TerminalGeometry::new(80, 24), options);
        let response = protocol.receive(&[IAC, SB, OPT_TERMINAL_TYPE, TTYPE_SEND, IAC, SE]);

        assert_eq!(
            response.commands,
            [
                IAC,
                SB,
                OPT_TERMINAL_TYPE,
                TTYPE_IS,
                b'x',
                b't',
                b'e',
                b'r',
                b'm',
                IAC,
                SE
            ]
        );
    }

    #[test]
    fn does_not_repeat_negotiation_replies_for_enabled_options() {
        let options = TerminalOptions::default();
        let mut protocol = TelnetProtocol::new(TerminalGeometry::new(80, 24), options);
        let response = protocol.receive(&[
            IAC,
            DO,
            OPT_TERMINAL_TYPE,
            IAC,
            DO,
            OPT_TERMINAL_TYPE,
            IAC,
            WILL,
            OPT_ECHO,
            IAC,
            WILL,
            OPT_ECHO,
        ]);

        assert_eq!(
            count_sequence(&response.commands, &[IAC, WILL, OPT_TERMINAL_TYPE]),
            1
        );
        assert_eq!(count_sequence(&response.commands, &[IAC, DO, OPT_ECHO]), 1);
    }

    #[test]
    fn initial_naws_request_sends_size_when_accepted() {
        let options = TerminalOptions::default();
        let mut protocol = TelnetProtocol::new(TerminalGeometry::new(80, 24), options);
        let initial = protocol.initial_commands();

        assert_eq!(count_sequence(&initial, &[IAC, WILL, OPT_NAWS]), 1);
        assert_eq!(count_sequence(&initial, &[IAC, SB, OPT_NAWS]), 0);

        let response = protocol.receive(&[IAC, DO, OPT_NAWS]);
        assert_eq!(
            count_sequence(&response.commands, &[IAC, WILL, OPT_NAWS]),
            0
        );
        assert_eq!(
            response.commands,
            [IAC, SB, OPT_NAWS, 0, 80, 0, 24, IAC, SE]
        );
    }

    #[test]
    fn sends_naws_on_resize() {
        let options = TerminalOptions::default();
        let mut protocol = TelnetProtocol::new(TerminalGeometry::new(80, 24), options);
        let _ = protocol.receive(&[IAC, DO, OPT_NAWS]);

        assert_eq!(
            protocol.resize(TerminalGeometry::new(132, 43)),
            [IAC, SB, OPT_NAWS, 0, 132, 0, 43, IAC, SE]
        );
    }

    #[test]
    fn binary_mode_preserves_client_newlines_but_still_escapes_iac() {
        let options = TerminalOptions::default();
        let mut protocol = TelnetProtocol::new(TerminalGeometry::new(80, 24), options);

        assert_eq!(protocol.encode_input(b"a\r", UTF_8), b"a\r\n");

        let response = protocol.receive(&[IAC, DO, OPT_BINARY]);
        assert_eq!(response.commands, [IAC, WILL, OPT_BINARY]);
        assert_eq!(
            protocol.encode_input(&[b'a', IAC, b'\r'], UTF_8),
            [b'a', IAC, IAC, b'\r']
        );
    }

    #[test]
    fn strips_nvt_cr_nul_unless_remote_binary_is_enabled() {
        let options = TerminalOptions::default();
        let mut protocol = TelnetProtocol::new(TerminalGeometry::new(80, 24), options.clone());

        assert_eq!(protocol.receive(b"a\r\0b").output, b"a\rb");

        let mut binary_protocol = TelnetProtocol::new(TerminalGeometry::new(80, 24), options);
        let response = binary_protocol.receive(&[IAC, WILL, OPT_BINARY]);
        assert_eq!(response.commands, [IAC, DO, OPT_BINARY]);
        assert_eq!(binary_protocol.receive(b"\r\0").output, b"\r\0");
    }

    #[test]
    fn normalizes_bare_remote_line_feeds_outside_binary_mode() {
        let options = TerminalOptions::default();
        let mut protocol = TelnetProtocol::new(TerminalGeometry::new(80, 24), options.clone());

        assert_eq!(
            protocol.receive(b"one\ntwo\r\nthree").output,
            b"one\r\ntwo\r\nthree"
        );
        assert_eq!(protocol.receive(b"\r").output, b"\r");
        assert_eq!(protocol.receive(b"\n").output, b"\n");

        let mut binary_protocol = TelnetProtocol::new(TerminalGeometry::new(80, 24), options);
        let response = binary_protocol.receive(&[IAC, WILL, OPT_BINARY]);
        assert_eq!(response.commands, [IAC, DO, OPT_BINARY]);
        assert_eq!(binary_protocol.receive(b"one\ntwo").output, b"one\ntwo");
    }
}
