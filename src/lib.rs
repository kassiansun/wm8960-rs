#![no_std]

mod regmap;

use embedded_hal::i2c::I2c;
use registers::*;
use regmap::{fields::*, RESET_VALUES};

/// Raw 7-bit register addresses, distinct from the codec's I2C device address.
pub use regmap::registers;

/// Shared I2C constants and typed descriptors for every register field in `wm8960.yaml`.
///
/// Descriptors are named `FIELDSET_FIELD` and apply to the unpacked 9-bit register value.
///
/// ```
/// use wm8960::{common, registers};
///
/// let bytes = [0x1e, 0x01];
/// let word = u16::from_be_bytes(bytes);
/// let register = ((word & common::WM8960_ADDR_MASK) >> 9) as u8;
/// let value = word & common::WM8960_REG_MASK;
/// assert_eq!((register, value), (registers::RESET, 1));
/// ```
///
/// ```
/// use wm8960::common::{Field, ANTI_POP1_POBCTRL};
///
/// let field: Field = ANTI_POP1_POBCTRL;
/// let mut value = 0x008;
/// value = (value & !field.mask) | ((1 << field.shift) & field.mask);
/// assert_eq!(value, 0x088);
/// ```
pub mod common {
    pub use crate::regmap::fields::*;
    pub use crate::{Field, WM8960_ADDR, WM8960_ADDR_MASK, WM8960_REG_MASK};
}

/// The codec's 7-bit I2C device address.
pub const WM8960_ADDR: usize = 0x1a;
/// The 9-bit register value mask in a packed I2C write word.
pub const WM8960_REG_MASK: u16 = 0x01FF;
/// The register address mask in a packed I2C write word, before shifting right by 9.
pub const WM8960_ADDR_MASK: u16 = !WM8960_REG_MASK;

const WM8960_PLL_N_MIN_VALUE: u32 = 6;
const WM8960_PLL_N_MAX_VALUE: u32 = 12;

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
pub struct Sysclk {
    pub source: SysclkSource,
    pub freq: u32,
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

#[derive(Copy, Clone)]
pub struct Format {
    pub mclk_freq: u32,
    pub sample_rate: SampleRate,
    pub bit_width: BitWidth,
}

pub struct Config {
    pub master: bool,
    pub protocol: Protocol,
    pub route: Route,
    pub sysclk: Sysclk,
    pub format: Format,
    pub speaker_en: bool,
}

/// A driver for the WM8960 Codec
pub struct WM8960<I2C> {
    i2c: I2C,
    regs: [u16; RESET_VALUES.len()],
}

impl<I2C: I2c> WM8960<I2C> {
    fn write_register(&mut self, addr: u8, value: u16) -> Result<(), I2C::Error> {
        let value = value & WM8960_REG_MASK;
        let bytes = ((u16::from(addr) << 9) | value).to_be_bytes();
        self.i2c.write(WM8960_ADDR as u8, &bytes)?;
        self.regs[usize::from(addr)] = value;
        Ok(())
    }

    fn update_register(&mut self, addr: u8, fields: &[(Field, u16)]) -> Result<(), I2C::Error> {
        // The codec does not support reading registers over I2C.
        let mut value = self.regs[usize::from(addr)];
        for (field, bits) in fields {
            value = (value & !field.mask) | ((bits << field.shift) & field.mask);
        }
        self.write_register(addr, value)
    }

    /// Setup a new WM8960 and initialize it
    pub fn new(i2c: I2C, cfg: &Config) -> Result<Self, I2C::Error> {
        let mut s = Self {
            i2c,
            regs: RESET_VALUES,
        };
        s.write_register(RESET, 1)?;

        s.update_register(
            PWR_MGMT1,
            &[
                (PWR_MGMT1_VMIDSEL, 0b01), // 50k
                (PWR_MGMT1_VREF, 1),
                (PWR_MGMT1_AINL, 1),
                (PWR_MGMT1_AINR, 1),
                (PWR_MGMT1_ADCL, 1),
                (PWR_MGMT1_ADCR, 1),
                (PWR_MGMT1_MICB, 1),
            ],
        )?;
        s.update_register(
            PWR_MGMT2,
            &[
                (PWR_MGMT2_DACR, 1),
                (PWR_MGMT2_DACL, 1),
                (PWR_MGMT2_LOUT1, 1),
                (PWR_MGMT2_ROUT1, 1),
                (PWR_MGMT2_SPKR, 1),
                (PWR_MGMT2_SPKL, 1),
            ],
        )?;
        s.update_register(
            PWR_MGMT3,
            &[
                (PWR_MGMT3_ROMIX, 1),
                (PWR_MGMT3_LOMIX, 1),
                (PWR_MGMT3_LMIC, 1),
                (PWR_MGMT3_RMIC, 1),
            ],
        )?;

        //TODO adc/dac same clock, verify
        s.write_register(AUD_INTF2, 0x40)?;

        s.set_route(cfg.route)?;
        s.set_protocol(cfg.protocol)?;
        let sysclk = if let SysclkSource::PLL = cfg.sysclk.source {
            s.set_internal_pll_cfg(
                cfg.format.mclk_freq,
                cfg.sysclk.freq,
                cfg.format.sample_rate as u32,
                cfg.format.bit_width as u32,
            )?;
            cfg.sysclk.freq
        } else {
            cfg.format.mclk_freq
        };
        if cfg.master {
            s.set_master_clock(
                sysclk,
                cfg.format.sample_rate as u32,
                cfg.format.bit_width as u32,
            )?;
        }
        s.set_master(cfg.master)?;

        s.write_register(ADDTL_CTL1, 0x0c0)?;
        s.write_register(ADDTL_CTL4, 0x40)?;
        s.write_register(LBYPASS, 0)?;
        s.write_register(RBYPASS, 0)?;
        s.write_register(LADC_VOL, 0x1c3)?;
        s.write_register(RADC_VOL, 0x1c3)?;
        s.write_register(LDAC_VOL, 0x1e0)?;
        s.write_register(RDAC_VOL, 0x1e0)?;
        s.write_register(LOUT1_VOL, 0x16f)?;
        s.write_register(ROUT1_VOL, 0x16f)?;
        s.write_register(LSPKR_VOL, 0x1ff)?;
        s.write_register(RSPKR_VOL, 0x1ff)?;
        s.update_register(ADAC_CTL1, &[(ADAC_CTL1_DACMU, 0)])?;

        s.set_format(
            sysclk,
            cfg.format.sample_rate as u32,
            cfg.format.bit_width as u32,
        )?;

        Ok(s)
    }

    /// Set the audio routing option
    pub fn set_route(&mut self, route: Route) -> Result<(), I2C::Error> {
        match route {
            Route::Bypass => {
                // Input3 to output mixer, volume 0dB
                self.write_register(LOUT_MIX, 0x80)?;
                self.write_register(ROUT_MIX, 0x80)?;
            }
            Route::Playback => {
                // DAC to output mixer, volume 0dB
                self.write_register(LOUT_MIX, 0x100)?;
                self.write_register(ROUT_MIX, 0x100)?;
                // Power On: DAC, Output Mix, and Line Out
                self.update_register(PWR_MGMT2, &[(PWR_MGMT2_DACR, 1)])?;
                self.update_register(PWR_MGMT3, &[(PWR_MGMT3_ROMIX, 1), (PWR_MGMT3_LOMIX, 1)])?;
                self.update_register(PWR_MGMT2, &[(PWR_MGMT2_LOUT1, 1), (PWR_MGMT2_ROUT1, 1)])?;
            }
            _ => {
                unimplemented!("TODO implement recording routes");
            }
        }
        Ok(())
    }

    /// Set the audio protocol (i2s, pcm, etc) option
    pub fn set_protocol(&mut self, protocol: Protocol) -> Result<(), I2C::Error> {
        self.update_register(AUD_INTF1, &[(AUD_INTF1_FORMAT, protocol as u16)])
    }

    /// Update PLL settings given input/output/sample rate/bitwidth settings
    /// Note this was ported from the mcux wm8960 driver (like much of this)
    /// and is unverified beyond matching the C code.
    fn set_internal_pll_cfg(
        &mut self,
        mut in_mclk: u32,
        out_mclk: u32,
        _sample_rate: u32,
        _bit_width: u32,
    ) -> Result<(), I2C::Error> {
        let pll_f2: u32 = out_mclk * 4;
        let mut prescale: u32 = 0;
        let mut div: u32 = 1;
        let mut frac_mode: u32 = 0;

        // disable PLL power
        self.update_register(PWR_MGMT2, &[(PWR_MGMT2_PLL_EN, 0)])?;
        self.update_register(CLOCKING1, &[(CLOCKING1_CLKSEL, 0)])?;

        let mut pll_n = pll_f2 / in_mclk;
        if pll_n < WM8960_PLL_N_MIN_VALUE {
            in_mclk >>= 1;
            prescale = 1;
            pll_n = pll_f2 / in_mclk;
            if pll_n < WM8960_PLL_N_MIN_VALUE {
                div = 2;
                pll_n = (pll_f2 * div) / in_mclk;
            }
        }

        if (pll_n < WM8960_PLL_N_MIN_VALUE) || (pll_n > WM8960_PLL_N_MAX_VALUE) {
            panic!("pll_n out of range {}", pll_n);
        }

        let pll_r = (((pll_f2 as u64) * (div as u64) * 1000) / ((in_mclk as u64) / 1000)) as u32;
        let pll_k =
            (((1 << 24) * ((pll_r as u64) - (pll_n as u64) * 1000 * 1000)) / 1000 / 1000) as u32;
        if pll_k != 0 {
            frac_mode = 1;
        }

        self.write_register(
            PLL_N,
            (frac_mode as u16) << 5 | (prescale as u16) << 4 | pll_n as u16 & 0xf,
        )?;
        self.write_register(PLL_K1, ((pll_k >> 16) & 0xff) as u16)?;
        self.write_register(PLL_K2, ((pll_k >> 8) & 0xff) as u16)?;
        self.write_register(PLL_K3, (pll_k & 0xff) as u16)?;
        self.update_register(PWR_MGMT2, &[(PWR_MGMT2_PLL_EN, 1)])?;
        let sysclk_div = if div == 1 { 0 } else { (div << 1) as u16 };
        self.update_register(
            CLOCKING1,
            &[(CLOCKING1_SYSCLKDIV, sysclk_div), (CLOCKING1_CLKSEL, 1)],
        )
    }

    /// Update master clock settings
    fn set_master_clock(
        &mut self,
        sysclk: u32,
        sample_rate: u32,
        bit_width: u32,
    ) -> Result<(), I2C::Error> {
        let clock_divider: u32 = (sysclk * 2) / (sample_rate * bit_width * 2);
        let reg_div = match clock_divider {
            2 => 0,
            3 => 1,
            4 => 2,
            6 => 3,
            8 => 4,
            11 => 5,
            12 => 6,
            16 => 7,
            22 => 8,
            24 => 9,
            32 => 10,
            44 => 11,
            48 => 12,
            _ => panic!("Invalid bit clock divider {}", clock_divider),
        };
        self.update_register(CLOCKING2, &[(CLOCKING2_BCLKDIV, reg_div)])
    }

    /// Set the master/slave flag for the codec
    fn set_master(&mut self, master: bool) -> Result<(), I2C::Error> {
        self.update_register(AUD_INTF1, &[(AUD_INTF1_MS, u16::from(master))])
    }

    /// Configure the audio format and dac/adc dividers
    fn set_format(
        &mut self,
        sysclk: u32,
        sample_rate: u32,
        bit_width: u32,
    ) -> Result<(), I2C::Error> {
        const DIVS: [u32; 7] = [256, 384, 512, 768, 1024, 1408, 1536];

        let mut div_idx: usize = usize::MAX;

        for i in 0..DIVS.len() {
            let rate = sysclk / DIVS[i];
            if rate == sample_rate {
                div_idx = i;
                break;
            }
        }
        if div_idx > DIVS.len() {
            panic!(
                "No dac/adc divider found for sysclk {}, sample_rate {}",
                sysclk, sample_rate
            );
        }

        /* Compute sample rate divider, dac and adc are the same sample rate */
        self.update_register(
            CLOCKING1,
            &[
                (CLOCKING1_DACDIV, div_idx as u16),
                (CLOCKING1_ADCDIV, div_idx as u16),
            ],
        )?;

        let wl = match bit_width {
            16 => 0b00,
            20 => 0b01,
            24 => 0b10,
            32 => 0b11,
            _ => panic!("Invalid bit width {}", bit_width),
        };

        self.update_register(AUD_INTF1, &[(AUD_INTF1_WL, wl)])
    }

    /// Update the output volume for out1 (line out)
    pub fn set_out_volume(&mut self, _lvol: u16, _rvol: u16) -> Result<(), I2C::Error> {
        // Currently this only rewrites the cached volume; the arguments are unused.
        self.write_register(LOUT1_VOL, self.regs[usize::from(LOUT1_VOL)])?;
        self.write_register(ROUT1_VOL, self.regs[usize::from(ROUT1_VOL)])
    }
}
