use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process;

// --定数・型定義--
type Pos = (i32, i32, i32);

// 方角定義 (0:北/Y+, 1:東/X+, 2:南/Y-, 3:西/X-)
const DX: [i32; 4] = [0, 1, 0, -1];
const DY: [i32; 4] = [1, 0, -1, 0];

#[derive(Debug, PartialEq, Clone)]
enum ObjectType {
    Door,
    StairsUp,
    StairsDown,
    Exit,
}

// 18bit純バイナリ文字列化 (X:6bit, Y:6bit, Z:6bit)
fn coords_to_18bit(x: i32, y: i32, z: i32) -> String {
    let x_bin = (x as u32) & 0x3F;
    let y_bin = (y as u32) & 0x3F;
    let z_bin = ((z + 32) as u32) & 0x3F; // 負のZ座標を6bit範囲にマッピング
    format!("{:06b}{:06b}{:06b}", x_bin, y_bin, z_bin)
}

// --ダンジョン管理--
struct Dungeon {
    floors: HashSet<Pos>, // 通行可能な床の座標セット
    objects: HashMap<Pos, ObjectType>,
    start_pos: Pos,
    opened_doors: HashSet<Pos>,
}

impl Dungeon {
    fn new() -> Self {
        let mut dungeon = Dungeon {
            floors: HashSet::new(),
            objects: HashMap::new(),
            start_pos: (0, 0, -3),
            opened_doors: HashSet::new(),
        };

        dungeon.load_map_file("maps/b3.txt", -3);
        dungeon.load_map_file("maps/b2.txt", -2);
        dungeon.load_map_file("maps/b1.txt", -1);
        dungeon.load_map_file("maps/g.txt", 0);

        dungeon
    }

    fn load_map_file<P: AsRef<Path>>(&mut self, filename: P, z: i32) {
        let file = match File::open(filename) {
            Ok(f) => f,
            Err(_) => return,
        };
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

        for (y_idx, line) in lines.iter().rev().enumerate() {
            for (x_idx, ch) in line.chars().enumerate() {
                let pos = (x_idx as i32, y_idx as i32, z);
                match ch {
                    ' ' | '.' => {
                        self.floors.insert(pos);
                    }
                    'P' => {
                        self.floors.insert(pos);
                        if z == -3 {
                            self.start_pos = pos;
                        }
                    }
                    'D' => {
                        self.floors.insert(pos);
                        self.objects.insert(pos, ObjectType::Door);
                    }
                    'U' => {
                        self.floors.insert(pos);
                        self.objects.insert(pos, ObjectType::StairsUp);
                    }
                    'd' => {
                        self.floors.insert(pos);
                        self.objects.insert(pos, ObjectType::StairsDown);
                    }
                    'E' => {
                        self.floors.insert(pos);
                        self.objects.insert(pos, ObjectType::Exit);
                    }
                    _ => {} // '#' や定義外の文字は floors に登録しない（＝壁扱い）
                }
            }
        }
    }

    fn is_wall(&self, x: i32, y: i32, z: i32) -> bool {
        let pos = (x, y, z);
        if !self.floors.contains(&pos) {
            return true;
        }
        if let Some(ObjectType::Door) = self.objects.get(&pos) {
            if !self.opened_doors.contains(&pos) {
                return true;
            }
        }
        false
    }

    fn find_object_pos(&self, target_type: ObjectType, target_z: i32) -> Option<Pos> {
        for (pos, obj_type) in &self.objects {
            if pos.2 == target_z && *obj_type == target_type {
                return Some(*pos);
            }
        }
        None
    }
}

// --プレーヤー状態管理--
struct Player {
    x: i32,
    y: i32,
    z: i32,
    facing: usize,
}

impl Player {
    fn new(x: i32, y: i32, z: i32) -> Self {
        Player {
            x,
            y,
            z,
            facing: 0,
        }
    }

    fn turn_right(&mut self) {
        self.facing = (self.facing + 1) % 4;
    }

    fn turn_left(&mut self) {
        self.facing = (self.facing + 3) % 4;
    }

    fn get_forward_pos(&self) -> Pos {
        (self.x + DX[self.facing], self.y + DY[self.facing], self.z)
    }

    fn get_backward_pos(&self) -> Pos {
        (self.x - DX[self.facing], self.y - DY[self.facing], self.z)
    }
}

// --ゲームエンジン--
struct GameEngine {
    dungeon: Dungeon,
    player: Player,
    last_used_stairs: Option<Pos>, // 直前に使用して着地した階段の座標
}

impl GameEngine {
    fn new() -> Self {
        let dungeon = Dungeon::new();
        let (sx, sy, sz) = dungeon.start_pos;
        let player = Player::new(sx, sy, sz);

        GameEngine {
            dungeon,
            player,
            last_used_stairs: None,
        }
    }

    fn print_log(&self, prefix: &str) {
        let bin_str = coords_to_18bit(self.player.x, self.player.y, self.player.z);
        println!("{}{}", prefix, bin_str);
    }

    fn check_nearby_object(&self) -> bool {
        for d in 0..4 {
            let nx = self.player.x + DX[d];
            let ny = self.player.y + DY[d];
            let pos = (nx, ny, self.player.z);

            if let Some(obj) = self.dungeon.objects.get(&pos) {
                match obj {
                    ObjectType::Door => {
                        if !self.dungeon.opened_doors.contains(&pos) {
                            return true;
                        }
                    }
                    ObjectType::StairsUp | ObjectType::StairsDown => return true,
                    _ => {}
                }
            }
        }
        false
    }

    fn prompt_yn(&self, msg: &str) -> bool {
        print!("{}", msg);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            return input.trim().to_lowercase() == "y";
        }
        false
    }

    fn process_command(&mut self, cmd: &str) {
        match cmd.trim().to_lowercase().as_str() {
            "f" => {
                let (nx, ny, nz) = self.player.get_forward_pos();
                let f_pos = (nx, ny, nz);

                if let Some(ObjectType::Door) = self.dungeon.objects.get(&f_pos) {
                    if !self.dungeon.opened_doors.contains(&f_pos) {
                        if self.prompt_yn("open door?[y/n] ") {
                            self.dungeon.opened_doors.insert(f_pos);
                            self.player.x = nx;
                            self.player.y = ny;
                            self.last_used_stairs = None; // 移動成功で階段通過フラグを解除
                            self.after_move();
                        } else {
                            let prefix = if self.check_nearby_object() { "!" } else { "" };
                            self.print_log(prefix);
                        }
                        return;
                    }
                }

                if self.dungeon.is_wall(nx, ny, nz) {
                    println!("c{}", coords_to_18bit(nx, ny, nz));
                    let prefix = if self.check_nearby_object() { "!" } else { "" };
                    self.print_log(prefix);
                } else {
                    self.player.x = nx;
                    self.player.y = ny;
                    self.last_used_stairs = None; // 移動成功で階段通過フラグを解除
                    self.after_move();
                }
            }
            "b" => {
                let (nx, ny, nz) = self.player.get_backward_pos();
                if self.dungeon.is_wall(nx, ny, nz) {
                    println!("c{}", coords_to_18bit(nx, ny, nz));
                    let prefix = if self.check_nearby_object() { "!" } else { "" };
                    self.print_log(prefix);
                } else {
                    self.player.x = nx;
                    self.player.y = ny;
                    self.last_used_stairs = None; // 移動成功で階段通過フラグを解除
                    self.after_move();
                }
            }
            "r" => {
                self.player.turn_right();
                let prefix = if self.check_nearby_object() { "!" } else { "" };
                self.print_log(prefix);
            }
            "l" => {
                self.player.turn_left();
                let prefix = if self.check_nearby_object() { "!" } else { "" };
                self.print_log(prefix);
            }
            _ => {
                println!("command not found");
            }
        }
    }

    fn after_move(&mut self) {
        let curr_pos = (self.player.x, self.player.y, self.player.z);
        let curr_obj = self.dungeon.objects.get(&curr_pos).cloned();

        // 1. 脱出（クリア）判定
        if let Some(ObjectType::Exit) = curr_obj {
            println!("\x1b[32m\n*** mission complete ***\x1b[0m");
            process::exit(0);
        }

        // 2. 階段判定
        // 移動直後に着地した段階（last_used_stairs と同じ座標）であればプロンプトを出さずにスキップ
        let is_just_landed = self.last_used_stairs == Some(curr_pos);

        if let Some(ObjectType::StairsUp) = curr_obj {
            if !is_just_landed {
                if self.prompt_yn("up stairs?[y/n] ") {
                    let next_z = self.player.z + 1;
                    let target_pos = self
                        .dungeon
                        .find_object_pos(ObjectType::StairsDown, next_z)
                        .unwrap_or((self.player.x, self.player.y, next_z));

                    self.player.x = target_pos.0;
                    self.player.y = target_pos.1;
                    self.player.z = target_pos.2;

                    // 着地先の階段座標を記録
                    self.last_used_stairs = Some(target_pos);

                    let prefix = if self.check_nearby_object() { "!" } else { "" };
                    self.print_log(prefix);
                    return;
                }
            }
            let prefix = if self.check_nearby_object() { "!" } else { "" };
            self.print_log(prefix);
            return;
        }

        if let Some(ObjectType::StairsDown) = curr_obj {
            if !is_just_landed {
                if self.prompt_yn("down stairs?[y/n] ") {
                    let next_z = self.player.z - 1;
                    let target_pos = self
                        .dungeon
                        .find_object_pos(ObjectType::StairsUp, next_z)
                        .unwrap_or((self.player.x, self.player.y, next_z));

                    self.player.x = target_pos.0;
                    self.player.y = target_pos.1;
                    self.player.z = target_pos.2;

                    // 着地先の階段座標を記録
                    self.last_used_stairs = Some(target_pos);

                    let prefix = if self.check_nearby_object() { "!" } else { "" };
                    self.print_log(prefix);
                    return;
                }
            }
            let prefix = if self.check_nearby_object() { "!" } else { "" };
            self.print_log(prefix);
            return;
        }

        // 3. 通常ログ表示
        let prefix = if self.check_nearby_object() { "!" } else { "" };
        self.print_log(prefix);
    }
}

// --メイン関数--
fn main() {
    println!("\x1b[32mPlease escape.");
    println!();
    println!("  f : Forward");
    println!("  b : Backward");
    println!("  r : Turn Right");
    println!("  l : Turn Left\x1b[0m");
    println!();
    print!("\x1b[32mPress Enter to start game...\x1b[0m");
    io::stdout().flush().unwrap();

    let mut dummy = String::new();
    let _ = io::stdin().read_line(&mut dummy);

    println!();

    let mut engine = GameEngine::new();

    // 初回ログ出力
    let prefix = if engine.check_nearby_object() { "!" } else { "" };
    engine.print_log(prefix);

    let stdin = io::stdin();
    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if stdin.read_line(&mut input).is_err() {
            break;
        }

        let cmd = input.trim();
        if cmd == "quit" || cmd == "exit" {
            break;
        }

        engine.process_command(cmd);
    }
}