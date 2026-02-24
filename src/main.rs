fn main() {
    println!("Hello, world!");
}
/**
 * 
 * cargo run -p gb-desktop -- \
  --rom "/Users/evanputnam/Workplace/gb-emulator/roms/Pokemon - Yellow Version - Special Pikachu Edition (USA, Europe).gb" \
  --boot-rom /absolute/path/to/dmg_boot.bin \
  --save-in /absolute/path/to/game.sav \
  --save-out /absolute/path/to/game.sav \
  --max-cycles 5000000 \
  --mute
 */