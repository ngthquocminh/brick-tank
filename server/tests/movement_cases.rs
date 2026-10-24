use serde_json::Value;
use server::rules::{box_overlap, held_from_code, step_move, Dir, TankMove};

#[test]
fn shared_movement_cases() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../../shared/movement_cases.json")).unwrap();
    let ints = |v: &Value| -> Vec<i32> { v.as_array().unwrap().iter().map(|n| n.as_i64().unwrap() as i32).collect() };
    for case in &cases {
        let name = case["name"].as_str().unwrap();
        let walls: Vec<Vec<i32>> = case["walls"].as_array().unwrap().iter().map(ints).collect();
        let b = ints(&case["bounds"]);
        let others: Vec<Vec<i32>> = case["others"].as_array().unwrap().iter().map(ints).collect();
        let s = ints(&case["start"]);
        let mut t = TankMove { x: s[0], y: s[1], dir: Dir::from_u8(s[2] as u8).unwrap(), cd: s[3] as u8 };
        let blocked = |x: i32, y: i32| {
            let wall = (0..3).any(|j| (0..3).any(|i| {
                walls.iter().any(|w| x + i >= w[0] && x + i < w[0] + w[2] && y + j >= w[1] && y + j < w[1] + w[3])
            }));
            let outside = x < b[0] || y < b[1] || x + 3 > b[2] || y + 3 > b[3];
            wall || outside || others.iter().any(|o| box_overlap(x, y, o[0], o[1]))
        };
        for (i, (input, expect)) in case["inputs"].as_array().unwrap().iter()
            .zip(case["expect"].as_array().unwrap()).enumerate()
        {
            let held = held_from_code(input.as_u64().unwrap() as u8).unwrap();
            step_move(&mut t, held, blocked);
            let e = ints(expect);
            assert_eq!(vec![t.x, t.y, t.dir as i32, t.cd as i32], e, "case '{name}', tick {}", i + 1);
        }
    }
}
