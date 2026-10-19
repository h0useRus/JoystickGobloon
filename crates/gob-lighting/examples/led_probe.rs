//! Read-only: every VKB HID collection, its feature reports and the LED report 0x59.
//! `cargo run -p gob-lighting --example led_probe`
fn main() {
    let collections = gob_lighting::hid::collections(gob_lighting::vkb::VENDOR_ID);
    if collections.is_empty() {
        println!("no VKB HID collections (hidden by HidHide from this exe?)");
    }
    for c in &collections {
        println!(
            "{} pid={:04X} usage={:04X}:{:04X} feature_len={} feature_ids={:02X?}\n    {}",
            c.product, c.product_id, c.usage_page, c.usage, c.feature_len, c.feature_ids, c.path
        );
        if !c.feature_ids.contains(&gob_lighting::vkb::REPORT_ID) {
            continue;
        }
        match gob_lighting::hid::get_feature(
            &c.path,
            gob_lighting::vkb::REPORT_ID,
            c.feature_len as usize,
        ) {
            Ok(report) => {
                println!("    0x59 head: {:02X?}", &report[..report.len().min(32)]);
                for led in gob_lighting::vkb::decode(&report) {
                    println!("    {led:?}");
                }
            }
            Err(e) => println!("    0x59 read failed: {e}"),
        }
    }
}
