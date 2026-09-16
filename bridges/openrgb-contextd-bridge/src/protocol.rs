//! OpenRGB SDK Network Protocol Parser & Serializer

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read, Write};

pub const OPENRGB_MAGIC: &[u8; 4] = b"ORGB";
pub const OPENRGB_DEFAULT_PORT: u16 = 6742;
pub const PROTOCOL_VERSION: u32 = 6;

pub const PKT_REQUEST_CONTROLLER_COUNT: u32 = 0;
pub const PKT_REQUEST_CONTROLLER_DATA: u32 = 1;
pub const PKT_REQUEST_PROTOCOL_VERSION: u32 = 40;
pub const PKT_SET_CLIENT_NAME: u32 = 50;

pub const PKT_UPDATE_LEDS: u32 = 1050;
pub const PKT_UPDATE_ZONE_LEDS: u32 = 1051;
pub const PKT_UPDATE_SINGLE_LED: u32 = 1052;
pub const PKT_SET_CUSTOM_MODE: u32 = 1100;

#[derive(Debug, Clone)]
pub struct Header {
    pub dev_id: u32,
    pub pkt_id: u32,
    pub pkt_size: u32,
}

impl Header {
    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Option<Self>> {
        let mut magic = [0u8; 4];
        match reader.read_exact(&mut magic) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        if &magic != OPENRGB_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid OpenRGB magic header",
            ));
        }

        let dev_id = reader.read_u32::<LittleEndian>()?;
        let pkt_id = reader.read_u32::<LittleEndian>()?;
        let pkt_size = reader.read_u32::<LittleEndian>()?;

        Ok(Some(Header {
            dev_id,
            pkt_id,
            pkt_size,
        }))
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(OPENRGB_MAGIC)?;
        writer.write_u32::<LittleEndian>(self.dev_id)?;
        writer.write_u32::<LittleEndian>(self.pkt_id)?;
        writer.write_u32::<LittleEndian>(self.pkt_size)?;
        Ok(())
    }
}

/// Helper to serialize OpenRGB string (u16 length prefix + null terminated string)
pub fn write_openrgb_string<W: Write>(writer: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    writer.write_u16::<LittleEndian>((bytes.len() + 1) as u16)?;
    writer.write_all(bytes)?;
    writer.write_u8(0)?; // Null byte
    Ok(())
}

/// Helper to build mock OpenRGB Controller Data block for contextd
pub fn build_mock_controller_data(protocol_ver: u32) -> Vec<u8> {
    let mut buf = Vec::new();

    // Data Size header placeholder
    let mut data = Vec::new();
    let mut writer = Cursor::new(&mut data);

    // Device type 0 (Motherboard/Generic)
    writer.write_i32::<LittleEndian>(0).unwrap();
    write_openrgb_string(&mut writer, "contextd Ambient Engine").unwrap(); // Name
    if protocol_ver >= 1 {
        write_openrgb_string(&mut writer, "Performative Nonsense").unwrap(); // Vendor
    }
    write_openrgb_string(&mut writer, "contextd Lighting Vibe").unwrap(); // Description
    write_openrgb_string(&mut writer, "1.0").unwrap(); // Version
    write_openrgb_string(&mut writer, "Virtual Socket").unwrap(); // Serial
    write_openrgb_string(&mut writer, "contextd-rgb-control").unwrap(); // Location

    // Modes Count = 1 (Direct Mode)
    writer.write_u16::<LittleEndian>(1).unwrap();
    writer.write_i32::<LittleEndian>(0).unwrap(); // Active mode index 0
    write_openrgb_string(&mut writer, "Direct").unwrap(); // Mode name
    writer.write_i32::<LittleEndian>(0).unwrap(); // Mode value
    writer.write_u32::<LittleEndian>(1).unwrap(); // Mode flags (Per LED color)
    writer.write_u32::<LittleEndian>(0).unwrap(); // Speed min
    writer.write_u32::<LittleEndian>(0).unwrap(); // Speed max
    writer.write_u32::<LittleEndian>(0).unwrap(); // Colors min
    writer.write_u32::<LittleEndian>(0).unwrap(); // Colors max
    writer.write_u32::<LittleEndian>(0).unwrap(); // Speed
    writer.write_u32::<LittleEndian>(0).unwrap(); // Direction
    writer.write_u32::<LittleEndian>(2).unwrap(); // Color mode PER_LED

    // Mode Colors Count = 0
    writer.write_u16::<LittleEndian>(0).unwrap();

    // Zones Count = 1
    writer.write_u16::<LittleEndian>(1).unwrap();
    write_openrgb_string(&mut writer, "Vibe Zone").unwrap(); // Zone name
    writer.write_i32::<LittleEndian>(0).unwrap(); // Zone type (Linear)
    writer.write_u32::<LittleEndian>(1).unwrap(); // LEDs min
    writer.write_u32::<LittleEndian>(64).unwrap(); // LEDs max
    writer.write_u32::<LittleEndian>(1).unwrap(); // LEDs count
    writer.write_u16::<LittleEndian>(0).unwrap(); // Matrix map size 0

    // LEDs Count = 1
    writer.write_u16::<LittleEndian>(1).unwrap();
    write_openrgb_string(&mut writer, "Main LED").unwrap();
    writer.write_u32::<LittleEndian>(0).unwrap(); // Value

    // Colors Count = 1
    writer.write_u16::<LittleEndian>(1).unwrap();
    writer.write_u8(255).unwrap(); // R
    writer.write_u8(255).unwrap(); // G
    writer.write_u8(255).unwrap(); // B
    writer.write_u8(0).unwrap(); // Unused

    let total_size = data.len() as u32;
    buf.write_u32::<LittleEndian>(total_size).unwrap();
    buf.extend_from_slice(&data);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_serialization() {
        let hdr = Header {
            dev_id: 0,
            pkt_id: 40,
            pkt_size: 4,
        };

        let mut buf = Vec::new();
        hdr.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(buf);
        let parsed = Header::read_from(&mut cursor).unwrap().unwrap();

        assert_eq!(parsed.dev_id, 0);
        assert_eq!(parsed.pkt_id, 40);
        assert_eq!(parsed.pkt_size, 4);
    }
}
