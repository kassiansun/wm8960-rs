# Wolfson WM8960 Codec Driver in Rust

A Rust driver for the common, though now obsolete Wolfson WM8960 commonly used on NXP development
boards.

The device can provides 48kHz 24bit audio ADC/DAC, a small builtin class d speaker amp, simple routing
and mixing, a I2S/TDM digital audio interface, along with an I2C control interface.

The I2C control interface has a total of 56 9bit registers with 7bit addresses (16bit total i2c transfers).

Only I2C writes are possible with the interface so the driver maintains an in memory mirror of the registers
requiring 112 bytes of memory.

The driver accepts any bus implementing `embedded_hal::i2c::I2c`. Use the methods
on `WM8960` to configure the codec. To inspect raw I2C writes, use
`wm8960::registers` for register addresses and `wm8960::common` for the I2C device
address, packet masks, and typed register fields such as `ANTI_POP1_POBCTRL`.
Each `Field` provides a mask and shift for the unpacked 9-bit register value.
Writes made outside the driver do not update its register cache.

## Regenerating the register map

After editing [wm8960.yaml](wm8960.yaml), run `cargo build --features codegen`.
The build script uses chiptool's register model to generate public register
addresses and typed fields, with private reset values. It automatically refreshes
[src/generated/regmap.rs](src/generated/regmap.rs) for plain `cargo build`.
Commit the generated changes along with the YAML changes.
