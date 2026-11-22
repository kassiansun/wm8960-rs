#![no_std]

pub mod regmap;

/// The codec's 7-bit I2C device address.
pub const WM8960_ADDR: usize = 0x1a;
/// The 9-bit register value mask in a packed I2C write word.
pub const WM8960_REG_MASK: u16 = 0x01FF;
/// The register address mask in a packed I2C write word, before shifting right by 9.
pub const WM8960_ADDR_MASK: u16 = !WM8960_REG_MASK;

/// A bit field within an unpacked 9-bit register value.
pub struct Field {
    /// Bit mask in its register position.
    pub mask: u16,
    /// Bit offset of the field from the least significant bit.
    pub shift: u8,
}

#[derive(Copy, Clone)]
pub enum Route {
    Bypass = 0,
    Playback = 1,
    PlaybackAndRecord = 2,
    Record = 3,
}

#[derive(Copy, Clone)]
pub enum Protocol {
    I2S = 2,
    LeftJustified = 1,
    RightJustified = 0,
    PCMA = 3,
    PCMB = 3 | (1 << 4),
}

#[derive(Copy, Clone)]
pub enum SysclkSource {
    Mclk = 0,
    PLL = 1,
}

#[derive(Copy, Clone)]
pub enum SampleRate {
    SR48000 = 48000,
}

#[derive(Copy, Clone)]
pub enum BitWidth {
    BW16 = 16,
    BW32 = 32,
}
