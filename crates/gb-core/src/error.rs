use thiserror::Error;

#[derive(Error, Debug)]
pub enum BusError {
    #[error("invalid address: {0:#06X}")]
    InvalidAddress(u16),
    #[error("cartridge error: {0}")]
    Cartridge(#[from] CartridgeError),
    #[error("joypad error: {0}")]
    Joypad(#[from] JoypadError),
    #[error("serial error: {0}")]
    Serial(#[from] SerialError),
}

#[derive(Error, Debug)]
pub enum CpuError {
    #[error("unsupported opcode: {0:#04X}")]
    UnsupportedOpcode(u8),
    #[error("unsupported CB opcode: {0:#04X}")]
    UnsupportedCbOpcode(u8),
    #[error("bus error: {0}")]
    Bus(#[from] BusError),
}

#[derive(Error, Debug)]
pub enum CartridgeError {
    #[error("rom is too small to contain a valid header")]
    RomTooSmall,
    #[error("unsupported cartridge type: {0:#04X}")]
    UnsupportedCartridgeType(u8),
    #[error("unsupported rom size code: {0:#04X}")]
    UnsupportedRomSizeCode(u8),
    #[error("unsupported ram size code: {0:#04X}")]
    UnsupportedRamSizeCode(u8),
    #[error("save ram size mismatch: expected {expected}, got {actual}")]
    SaveRamSizeMismatch { expected: usize, actual: usize },
    #[error("rtc state is required for this cartridge")]
    MissingRtcState,
}

#[derive(Error, Debug)]
pub enum ClockError {
    #[error("clock overflow")]
    Overflow,
}

#[derive(Error, Debug)]
pub enum TimerError {
    #[error("invalid timer state")]
    InvalidState,
}

#[derive(Error, Debug)]
pub enum PpuError {
    #[error("invalid ppu state")]
    InvalidState,
}

#[derive(Error, Debug)]
pub enum DmaError {
    #[error("dma transfer source overflow")]
    SourceOverflow,
    #[error("bus error during dma: {0}")]
    Bus(#[from] BusError),
}

#[derive(Error, Debug)]
pub enum JoypadError {
    #[error("invalid joypad button state")]
    InvalidButtonState,
}

#[derive(Error, Debug)]
pub enum SerialError {
    #[error("invalid serial state")]
    InvalidState,
}

#[derive(Error, Debug)]
pub enum BootError {
    #[error("boot rom must contain exactly 256 bytes, got {0}")]
    InvalidLength(usize),
}

#[derive(Error, Debug)]
pub enum ApuError {
    #[error("invalid apu state")]
    InvalidState,
}

#[derive(Error, Debug)]
pub enum StateError {
    #[error("snapshot payload is too large to encode safely: {actual} bytes")]
    PayloadTooLarge { actual: u64 },
    #[error("snapshot is too small: expected at least {expected} bytes, got {actual}")]
    SnapshotTooSmall { expected: usize, actual: usize },
    #[error("invalid snapshot magic: expected 'GBST', got {actual:02X?}")]
    InvalidMagic { actual: [u8; 4] },
    #[error("unsupported snapshot version: {0}")]
    UnsupportedVersion(u16),
    #[error("snapshot payload length mismatch: expected {expected}, got {actual}")]
    PayloadLengthMismatch { expected: usize, actual: usize },
    #[error("snapshot checksum mismatch: expected {expected:#010X}, got {actual:#010X}")]
    ChecksumMismatch { expected: u32, actual: u32 },
    #[error("snapshot decode failed: {0}")]
    Decode(String),
}

#[derive(Error, Debug)]
pub enum GbError {
    #[error("cpu error: {0}")]
    Cpu(#[from] CpuError),
    #[error("bus error: {0}")]
    Bus(#[from] BusError),
    #[error("cartridge error: {0}")]
    Cartridge(#[from] CartridgeError),
    #[error("timer error: {0}")]
    Timer(#[from] TimerError),
    #[error("clock error: {0}")]
    Clock(#[from] ClockError),
    #[error("ppu error: {0}")]
    Ppu(#[from] PpuError),
    #[error("apu error: {0}")]
    Apu(#[from] ApuError),
    #[error("dma error: {0}")]
    Dma(#[from] DmaError),
    #[error("joypad error: {0}")]
    Joypad(#[from] JoypadError),
    #[error("serial error: {0}")]
    Serial(#[from] SerialError),
    #[error("boot error: {0}")]
    Boot(#[from] BootError),
    #[error("state error: {0}")]
    State(#[from] StateError),
}
