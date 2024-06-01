pub fn add_brightness(brightness: u8, delta: i16) -> u8 {
    (brightness as i16 + delta).clamp(0, 100) as u8
}
