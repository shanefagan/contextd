//! OpenRGB SDK network protocol: the parts the bridge needs.
//!
//! Layouts follow the OpenRGB SDK documentation and the openrgb-python
//! client. All integers are little-endian. Colors are 4 bytes: R, G, B and
//! one padding byte. Where a payload starts with `data_size`, that size
//! includes its own 4 bytes.

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read, Write};

pub const OPENRGB_MAGIC: &[u8; 4] = b"ORGB";
pub const OPENRGB_DEFAULT_PORT: u16 = 6742;

/// Highest protocol version whose controller-data layout we serialize.
/// v1 added the vendor string, v3 mode brightness, v4 zone segments; v2
/// and v4 also added profile and plugin list requests, which we answer
/// with empty lists.
pub const PROTOCOL_VERSION: u32 = 4;

/// Largest packet payload accepted from a client. A real UPDATELEDS for
/// our single-LED controller is a few bytes.
pub const MAX_PACKET_SIZE: u32 = 64 * 1024;

pub const PKT_REQUEST_CONTROLLER_COUNT: u32 = 0;
pub const PKT_REQUEST_CONTROLLER_DATA: u32 = 1;
pub const PKT_REQUEST_PROTOCOL_VERSION: u32 = 40;
pub const PKT_SET_CLIENT_NAME: u32 = 50;
pub const PKT_REQUEST_PROFILE_LIST: u32 = 150;
pub const PKT_REQUEST_PLUGIN_LIST: u32 = 200;

pub const PKT_UPDATE_LEDS: u32 = 1050;
pub const PKT_UPDATE_ZONE_LEDS: u32 = 1051;
pub const PKT_UPDATE_SINGLE_LED: u32 = 1052;
pub const PKT_SET_CUSTOM_MODE: u32 = 1100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub dev_id: u32,
    pub pkt_id: u32,
    pub pkt_size: u32,
}

pub const HEADER_SIZE: usize = 16;

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

/// Builds a complete packet (header + payload).
pub fn packet(dev_id: u32, pkt_id: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
    Header {
        dev_id,
        pkt_id,
        pkt_size: payload.len() as u32,
    }
    .write_to(&mut out)
    .expect("writing to a Vec cannot fail");
    out.extend_from_slice(payload);
    out
}

pub type Rgb = [u8; 3];

fn read_color(c: &mut Cursor<&[u8]>) -> io::Result<Rgb> {
    let mut buf = [0u8; 4];
    c.read_exact(&mut buf)?;
    Ok([buf[0], buf[1], buf[2]])
}

fn read_colors(c: &mut Cursor<&[u8]>) -> io::Result<Vec<Rgb>> {
    let count = c.read_u16::<LittleEndian>()?;
    (0..count).map(|_| read_color(c)).collect()
}

/// Checks the leading `data_size` against the payload length.
fn check_data_size(c: &mut Cursor<&[u8]>, payload_len: usize) -> io::Result<()> {
    let size = c.read_u32::<LittleEndian>()? as usize;
    if size > payload_len || size < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("data_size {size} does not fit payload of {payload_len} bytes"),
        ));
    }
    Ok(())
}

/// The LED colors carried by an LED update packet, or `None` for packets
/// that carry no colors.
///
/// - UPDATELEDS: `u32 data_size, u16 num_colors, colors`
/// - UPDATEZONELEDS: `u32 data_size, u32 zone_idx, u16 num_colors, colors`
/// - UPDATESINGLELED: `i32 led_idx, color`
pub fn parse_led_update(pkt_id: u32, payload: &[u8]) -> io::Result<Option<Vec<Rgb>>> {
    let mut c = Cursor::new(payload);
    match pkt_id {
        PKT_UPDATE_LEDS => {
            check_data_size(&mut c, payload.len())?;
            read_colors(&mut c).map(Some)
        }
        PKT_UPDATE_ZONE_LEDS => {
            check_data_size(&mut c, payload.len())?;
            let _zone = c.read_u32::<LittleEndian>()?;
            read_colors(&mut c).map(Some)
        }
        PKT_UPDATE_SINGLE_LED => {
            let _led = c.read_i32::<LittleEndian>()?;
            read_color(&mut c).map(|rgb| Some(vec![rgb]))
        }
        _ => Ok(None),
    }
}

/// Averages LED colors into the single ambient color contextd stores.
pub fn mean_color(colors: &[Rgb]) -> Option<Rgb> {
    if colors.is_empty() {
        return None;
    }
    let n = colors.len() as u32;
    let mut sum = [0u32; 3];
    for c in colors {
        for i in 0..3 {
            sum[i] += c[i] as u32;
        }
    }
    Some(sum.map(|s| ((s + n / 2) / n) as u8))
}

/// Helper to serialize OpenRGB string (u16 length prefix + null terminated string)
pub fn write_openrgb_string<W: Write>(writer: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    writer.write_u16::<LittleEndian>((bytes.len() + 1) as u16)?;
    writer.write_all(bytes)?;
    writer.write_u8(0)?;
    Ok(())
}

/// Reply payload for REQUEST_PROFILE_LIST and REQUEST_PLUGIN_LIST: an empty
/// list (`u32 data_size, u16 count`).
pub fn empty_list_reply() -> Vec<u8> {
    let mut buf = Vec::new();
    buf.write_u32::<LittleEndian>(6).unwrap();
    buf.write_u16::<LittleEndian>(0).unwrap();
    buf
}

/// The controller-data block for the single virtual contextd controller,
/// laid out for `protocol_ver`.
pub fn build_controller_data(protocol_ver: u32, current: Rgb) -> Vec<u8> {
    let mut w = Vec::new();

    w.write_i32::<LittleEndian>(0).unwrap(); // Device type: motherboard/generic
    write_openrgb_string(&mut w, "contextd Ambient Engine").unwrap(); // Name
    if protocol_ver >= 1 {
        write_openrgb_string(&mut w, "Performative Nonsense").unwrap(); // Vendor
    }
    write_openrgb_string(&mut w, "contextd Lighting Vibe").unwrap(); // Description
    write_openrgb_string(&mut w, env!("CARGO_PKG_VERSION")).unwrap(); // Version
    write_openrgb_string(&mut w, "Virtual Socket").unwrap(); // Serial
    write_openrgb_string(&mut w, "contextd-rgb-control").unwrap(); // Location

    // Modes: one "Direct" mode
    w.write_u16::<LittleEndian>(1).unwrap(); // Mode count
    w.write_i32::<LittleEndian>(0).unwrap(); // Active mode
    write_openrgb_string(&mut w, "Direct").unwrap();
    w.write_i32::<LittleEndian>(0).unwrap(); // Mode value
    w.write_u32::<LittleEndian>(1 << 5).unwrap(); // Flags: MODE_FLAG_HAS_PER_LED_COLOR
    w.write_u32::<LittleEndian>(0).unwrap(); // Speed min
    w.write_u32::<LittleEndian>(0).unwrap(); // Speed max
    if protocol_ver >= 3 {
        w.write_u32::<LittleEndian>(0).unwrap(); // Brightness min
        w.write_u32::<LittleEndian>(0).unwrap(); // Brightness max
    }
    w.write_u32::<LittleEndian>(0).unwrap(); // Colors min
    w.write_u32::<LittleEndian>(0).unwrap(); // Colors max
    w.write_u32::<LittleEndian>(0).unwrap(); // Speed
    if protocol_ver >= 3 {
        w.write_u32::<LittleEndian>(0).unwrap(); // Brightness
    }
    w.write_u32::<LittleEndian>(0).unwrap(); // Direction
    w.write_u32::<LittleEndian>(1).unwrap(); // Color mode: MODE_COLORS_PER_LED
    w.write_u16::<LittleEndian>(0).unwrap(); // Mode colors count

    // Zones: one linear zone with one LED
    w.write_u16::<LittleEndian>(1).unwrap();
    write_openrgb_string(&mut w, "Vibe Zone").unwrap();
    w.write_i32::<LittleEndian>(1).unwrap(); // Zone type: linear
    w.write_u32::<LittleEndian>(1).unwrap(); // LEDs min
    w.write_u32::<LittleEndian>(1).unwrap(); // LEDs max
    w.write_u32::<LittleEndian>(1).unwrap(); // LEDs count
    w.write_u16::<LittleEndian>(0).unwrap(); // Matrix map size
    if protocol_ver >= 4 {
        w.write_u16::<LittleEndian>(0).unwrap(); // Segment count
    }

    // LEDs
    w.write_u16::<LittleEndian>(1).unwrap();
    write_openrgb_string(&mut w, "Main LED").unwrap();
    w.write_u32::<LittleEndian>(0).unwrap(); // Value

    // Colors
    w.write_u16::<LittleEndian>(1).unwrap();
    w.write_all(&[current[0], current[1], current[2], 0])
        .unwrap();

    let mut buf = Vec::with_capacity(w.len() + 4);
    buf.write_u32::<LittleEndian>(w.len() as u32 + 4).unwrap(); // includes itself
    buf.extend_from_slice(&w);
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
        assert_eq!(parsed, hdr);
    }

    fn update_leds(colors: &[Rgb]) -> Vec<u8> {
        let mut body = Vec::new();
        body.write_u16::<LittleEndian>(colors.len() as u16).unwrap();
        for c in colors {
            body.extend_from_slice(&[c[0], c[1], c[2], 0]);
        }
        let mut p = Vec::new();
        p.write_u32::<LittleEndian>(body.len() as u32 + 4).unwrap();
        p.extend(body);
        p
    }

    #[test]
    fn parses_update_leds_as_openrgb_python_sends_it() {
        // struct.pack("I", size) + struct.pack("H", 1) + bytes([255, 0, 0, 0])
        let p = update_leds(&[[255, 0, 0]]);
        assert_eq!(p, [10, 0, 0, 0, 1, 0, 255, 0, 0, 0]);
        assert_eq!(
            parse_led_update(PKT_UPDATE_LEDS, &p).unwrap(),
            Some(vec![[255, 0, 0]])
        );
    }

    #[test]
    fn parses_zone_and_single_led() {
        let mut z = Vec::new();
        z.write_u32::<LittleEndian>(14).unwrap();
        z.write_u32::<LittleEndian>(0).unwrap();
        z.write_u16::<LittleEndian>(1).unwrap();
        z.extend_from_slice(&[0, 255, 0, 0]);
        assert_eq!(
            parse_led_update(PKT_UPDATE_ZONE_LEDS, &z).unwrap(),
            Some(vec![[0, 255, 0]])
        );

        let s = [0, 0, 0, 0, 0, 0, 255, 0];
        assert_eq!(
            parse_led_update(PKT_UPDATE_SINGLE_LED, &s).unwrap(),
            Some(vec![[0, 0, 255]])
        );
    }

    #[test]
    fn rejects_truncated_payloads() {
        let mut p = update_leds(&[[1, 2, 3], [4, 5, 6]]);
        p.truncate(p.len() - 2);
        assert!(parse_led_update(PKT_UPDATE_LEDS, &p).is_err());
        assert!(parse_led_update(PKT_UPDATE_LEDS, &[200, 0, 0, 0, 1, 0]).is_err());
    }

    #[test]
    fn custom_mode_has_no_colors() {
        assert_eq!(parse_led_update(PKT_SET_CUSTOM_MODE, &[]).unwrap(), None);
    }

    #[test]
    fn mean_of_colors() {
        assert_eq!(mean_color(&[[255, 0, 0], [0, 0, 255]]), Some([128, 0, 128]));
        assert_eq!(mean_color(&[]), None);
    }

    #[test]
    fn controller_data_size_includes_itself() {
        for v in 0..=PROTOCOL_VERSION {
            let d = build_controller_data(v, [1, 2, 3]);
            let size = u32::from_le_bytes(d[0..4].try_into().unwrap()) as usize;
            assert_eq!(size, d.len(), "protocol {v}");
        }
        // v3 adds 12 bytes of brightness fields, v4 a 2-byte segment count.
        let len = |v| build_controller_data(v, [0, 0, 0]).len();
        assert_eq!(len(3) - len(2), 12);
        assert_eq!(len(4) - len(3), 2);
    }
}
