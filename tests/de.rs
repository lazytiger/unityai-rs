use std::fs::File;
use std::io::Read;

use serde::Deserialize;

use unityai::serde::Vector3f;

#[derive(Deserialize, Debug)]
struct NavMeshData {
    m_NavMeshTiles: Vec<NavMeshTileData>,
    m_NavMeshBuildSettings: NavMeshBuildSettings,
    m_HeightMeshes: Vec<HeightMeshData>,
}

#[derive(Deserialize, Debug)]
struct NavMeshTileData {
    m_MeshData: Vec<u8>,
    //m_Hash: Hash128,
}

#[derive(Deserialize, Debug)]
struct Hash128 {
    bytes: [u8; 16],
}

#[derive(Deserialize, Debug)]
struct NavMeshBuildSettings {
    agentTypeID: i32,
    agentRadius: f32,
    agentHeight: f32,
    agentSlope: f32,
    agentClimb: f32,
    ledgeDropHeight: f32,
    maxJumpAcrossDistance: f32,
    minRegionArea: f32,
    manualCellSize: usize,
    tileSize: usize,
    accuratePlacement: i32,
}

#[derive(Deserialize, Debug)]
struct HeightMeshData {
    m_Vertices: Vec<Vector3f>,
    m_Indices: Vec<u32>,
}

fn init_log() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}:{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S%.6f]"),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

#[test]
fn test_de() {
    init_log().expect("init_log");
    let mut file = File::open("tests/Navmesh.asset.txt").expect("open file");
    //let mut file = File::open("tests/CSZ.asset.txt").expect("open file");
    //let mut file = File::open("tests/Obstacle.txt").expect("open file");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).expect("read_to_end");
    let data: NavMeshData =
        unityai::serde::from_str(unsafe { String::from_utf8_unchecked(buffer) }.as_str())
            .expect("deserialize NavMeshData");
    println!("data is {:?}", data);
}
