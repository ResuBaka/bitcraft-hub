use std::error::Error;
use std::fs::File;
use csv::ReaderBuilder;
use serde::Deserialize;
use plotters::prelude::*;

#[derive(Debug, Deserialize)]
struct ChunkRecord { //32x32
    index: u64,
    x: u32,
    y: u32,
    dimension: u32,
    biome: String,
    _biome_density: String,
    _elevation: String,
    water_level: String,
    water_type: String,
    _zoning: String,
    original_elevation: String,
}

fn parse_array_field(field: &str) -> Vec<u32> {
    field
        .trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .collect()
}

fn water_color(water_type: u32) -> RGBColor {
    let water_type_u8: u8 = (water_type & 0xFF) as u8;
    match water_type_u8 {
        0 => RGBColor(50, 137, 220),   // Ground - Blue // AKA Coastline
        1 => RGBColor(70, 130, 180),  // Lake - Steel Blue
        2 => RGBColor(30, 144, 255),  // River - Dodger Blue
        3 => RGBColor(0, 132, 186),   // Ocean - Sea Blue
        4 => RGBColor(0, 105, 148),  // Ocean Biome - Ocean Blue
        5 => RGBColor(47, 79, 79),    // Swamp - Dark Slate Gray
        _ => RGBColor(25, 25, 112),   // Unknown - Midnight Blue
    }
}

fn biome_color(biome_id: u32) -> RGBColor {
    let biome_id_u8: u8 = (biome_id & 0xFF) as u8;
    match biome_id_u8 {
        0 => RGBColor(255, 25, 0),        // Dev Chunks - RED
        1 => RGBColor(34, 139, 34),       // Calm Forest - Forest Green
        2 => RGBColor(0, 100, 0),         // Pine Woods - Dark Green
        3 => RGBColor(1, 1, 1),     // Snowy Peaks - Alice Blue
        4 => RGBColor(130, 230, 130),     // Breezy Plains - Light Green
        5 => RGBColor(210, 105, 30),      // Autumn Forest - Orange
        6 => RGBColor(150, 0, 120),       // Tundra - Pinky Purple
        7 => RGBColor(237, 201, 175),     // Desert - Sandy Brown
        8 => RGBColor(55, 110, 87),        // Swamp - Dark Slate Gray
        9 => RGBColor(112, 128, 144),     // Canyon(Rocky Gardens) - Slate Gray
        10 => RGBColor(255, 240, 200),        // Ocean - Sand Bar
        11 => RGBColor(220, 255, 110),    // Safe Meadows - Light Green
        12 => RGBColor(105, 105, 105),    // Cave - Dim Gray
        13 => RGBColor(255, 100, 30),      // Jungle - Forest Green
        14 => RGBColor(255, 62, 45),      // Sapwoods - Sienna
        _ => RGBColor(0, 0, 0),           // Unknown - Black
    }
}


fn main() -> Result<(), Box<dyn Error>> {
    let file = File::open("dev3.csv")?;
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .from_reader(file);

    let mut data = Vec::new();

    for result in rdr.deserialize() {
        let record: ChunkRecord = result?;

        if record.dimension != 1 {
            continue;
        }

        let biomes = parse_array_field(&record.biome);
        let water_levels = parse_array_field(&record.water_level);
        let water_types = parse_array_field(&record.water_type);
        let original_elevations = parse_array_field(&record.original_elevation);

        data.push((record.x, record.y, biomes, water_levels, water_types, original_elevations));
    }

    let min_x = data.iter().map(|(x, _, _, _, _, _)| *x).min().unwrap_or(0);
    let min_y = data.iter().map(|(_, y, _, _, _, _)| *y).min().unwrap_or(0);
    let max_x = data.iter().map(|(x, _, _, _, _, _)| *x).max().unwrap_or(0);
    let max_y = data.iter().map(|(_, y, _, _, _, _)| *y).max().unwrap_or(0);

    let chunk_size = 1;
    let image_width = (max_x - min_x + 1) * chunk_size * 32;
    let image_height = (max_y - min_y + 1) * chunk_size * 32;

    let root = BitMapBackend::new("terrain_map.png", (image_width, image_height)).into_drawing_area();
    root.fill(&WHITE)?;

    for (x, y, biomes, water_levels, water_types, original_elevations) in data {
        for i in 0usize..1024 {
            let chunk_x: u32 = (i % 32) as u32;
            let chunk_y: u32 = (i / 32) as u32;
            let is_land = match (original_elevations.get(i), water_levels.get(i)) {
                (Some(&elev), Some(&water)) => elev > water,
                _ => false,
            };
            
            let landcolor = biome_color(*biomes.get(i).unwrap_or(&0));
            let elev = *original_elevations.get(i).unwrap_or(&0);
            let factor = elev.saturating_sub(25).min(255) as u8;
            let heightcolor = RGBColor(factor, factor, factor);

            let brightened_landcolor = RGBColor(
                landcolor.0.saturating_add(heightcolor.0).min(255),
                landcolor.1.saturating_add(heightcolor.1).min(255),
                landcolor.2.saturating_add(heightcolor.2).min(255),
            );

            let color = if is_land {
                brightened_landcolor
            } else {
                water_color(*water_types.get(i).unwrap_or(&0))
            };
            // let color = biome_color(*biomes.get(0).unwrap_or(&0));


            let x_pos = (x - min_x) * chunk_size * 32 + chunk_x;
            let y_pos = (max_y - y) * chunk_size * 32 + (31 - chunk_y);

            root.draw(&Rectangle::new(
                [(x_pos as i32, y_pos as i32), ((x_pos + chunk_size) as i32, (y_pos + chunk_size) as i32)],
                color.filled(),
            ))?;
        }
    }

    root.present()?;
    println!("Map generated: terrain_map.png");

    Ok(())
}




























// use std::fs;
// use std::path::Path;
// use hexx::hex;
// use image::{GenericImage, ImageBuffer, Pixel, Rgb, Rgba, RgbImage};
// use imageproc::drawing::{draw_line_segment_mut, draw_polygon_mut};
// use hexx::{shapes, *};
// use imageproc::point::Point;

// /// World size of the hexagons (outer radius)
// const HEX_SIZE: Vec2 = Vec2::splat(60.0);

// fn main() {
//     let path = Path::new("./orig");
//     let temp_path = Path::new("./bioms");
//     let paths: Vec<_> = fs::read_dir(path).unwrap().collect();
//     let mut chunk_coords = Vec::new();

//     for path in &paths {
//         let file_name = path.as_ref().unwrap().path().display().to_string();
//         let coords: Vec<i32> = file_name.replace("./orig/prod_terrain_chunk_1_", "").split("_").map(|s| s.parse().unwrap()).collect();
//         chunk_coords.push(coords);
//     }

//     let max_chunk_coord: Vec<i32> = chunk_coords.iter().fold(vec![0, 0], |acc, x| {
//         vec![acc[0].max(x[0]), acc[1].max(x[1])]
//     });
//     let mut full_img = ImageBuffer::new((32 + max_chunk_coord[0] * 32) as u32, (32 + max_chunk_coord[1] * 32) as u32);

//     for path in paths {
//         let path = path.unwrap().path();
//         let file = fs::read(&path).unwrap();
//         let file_name = path.display().to_string();
//         let ch_coords: Vec<i32> = file_name.replace("./orig/prod_terrain_chunk_1_", "").split("_").map(|s| s.parse().unwrap()).collect();

//         let start_info = &file[0..20];
//         let bioms = &file[20..4116];
//         let heightmap_current = &file[4116..6164];
//         let water_map_12 = &file[6164..8212];
//         let strange_part = &file[8212..9236];
//         let heightmap_original = &file[9236..11284];
//         // let strange_info = &file[11284..16404];

//         // for chunk in bioms.chunks(4) {
//         //     let r = chunk[0];
//         //     let g = chunk[1];
//         //     let b = chunk[2];
//         //     let a = chunk[3];
//         // println!("{r} {g} {b} {a}");
//         // }

//         render_bioms(bioms, &mut full_img, &ch_coords);
//         render_water(water_map_12, &mut full_img, &ch_coords);
//         render_land(heightmap_current, &mut full_img, &ch_coords);

//     }

//     full_img = image::imageops::flip_vertical(&full_img);
//     full_img.save(temp_path.join("output.png")).unwrap();

//     let layout = HexLayout {
//         hex_size: HEX_SIZE,
//         ..Default::default()
//     };

//     const image_size: Vec2 = Vec2::splat(4096.0);

//     // Create an image buffer
//     let mut imgage = RgbImage::new(image_size.x as u32, image_size.y as u32);

//     let white = Rgb([255u8, 255u8, 255u8]);
//     let blue  = Rgb([0u8,   0u8,   255u8]);


//     // image background
//     for (x, y, pixel) in imgage.enumerate_pixels_mut() {
//         *pixel = white;
//     }

//     let full_map_radius = image_size.x / HEX_SIZE.x ;

//     shapes::hexagon(Hex {
//         x: 0,
//         y: 0
//     }, full_map_radius as u32).for_each(|hex| {
//         let pos = layout.hex_to_world_pos(hex);
//         let corners = layout.hex_corners(hex);
//         let line_thickness = 10; // Set your desired line thickness
//         for i in 0..6 {
//             let p1 = corners[i];
//             let p2 = corners[(i + 1) % 6];
//             let perp = Vec2::new(p2.y - p1.y, p1.x - p2.x).normalize() * line_thickness as f32 / 2.0;
//             let p1_inner = Vec2::new(p1.x as f32, p1.y as f32) - perp;
//             let p2_inner = Vec2::new(p2.x as f32, p2.y as f32) - perp;
//             let p1_outer = Vec2::new(p1.x as f32, p1.y as f32) + perp;
//             let p2_outer = Vec2::new(p2.x as f32, p2.y as f32) + perp;
//             let polygon = vec![
//                 Point::new(p1_inner.x as i32, p1_inner.y as i32),
//                 Point::new(p2_inner.x as i32, p2_inner.y as i32),
//                 Point::new(p2_outer.x as i32, p2_outer.y as i32),
//                 Point::new(p1_outer.x as i32, p1_outer.y as i32),
//             ];
//             draw_polygon_mut(&mut imgage, &polygon, blue);
//         }
//     });

//     // Iterate over the grid
//     // for hex in grid.iter() {
//     //     // Calculate the corners of the hexagon
//     //     let corners = LayoutTool::polygon_corners(LAYOUT_ORIENTATION_POINTY, hex);
//     //
//     //     // Draw lines between the corners on the image buffer
//     //     for i in 0..6 {
//     //         let p1 = corners[i];
//     //         let p2 = corners[(i + 1) % 6];
//     //         draw_line_segment_mut(&mut img, (p1.x, p1.y), (p2.x, p2.y), Rgba([255, 255, 255, 255]));
//     //     }
//     // }

//     // Save the image buffer to a file
//     imgage.save(temp_path.join("grid.png")).unwrap();


// }

// fn random_color_rgba() -> Rgba<u8> {
//     let r = rand::random::<u8>();
//     let g = rand::random::<u8>();
//     let b = rand::random::<u8>();
//     Rgba([r, g, b, 255])
// }

// fn random_color_rgb() -> Rgb<u8> {
//     let r = rand::random::<u8>();
//     let g = rand::random::<u8>();
//     let b = rand::random::<u8>();
//     Rgb([r, g, b])
// }



// fn render_bioms(data: &[u8], full_img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ch_coords: &[i32]) {
//     let mut y = 0;
//     let mut x = 0;
//     let mut img = ImageBuffer::new(32, 32);
//     let (ch_x, ch_y) = (ch_coords[0] as u32, ch_coords[1] as u32);

//     for chunk in data.chunks(4) {
//         let r = chunk[0];
//         let g = chunk[1];
//         let b = chunk[2];
//         let a = chunk[3];

//         let color = match (r, g, b, a) {
//             (10, 0, 0, 0) => Rgba([0xff, 0xA0, 0x00, 0xff]),
//             (9, 0, 0, 0) => Rgba([0xAD, 0xA5, 0xB2, 0xff]),
//             (8, 9, 0, 0) => Rgba([0xff, 0x89, 0x00, 0xff]),
//             (8, 0, 0, 0) => Rgba([0x6F, 0xA2, 0x9C, 0xff]),
//             (9, 1, 0, 0) => Rgba([0xAD, 0xA5, 0xB2, 0xff]),
//             (1, 9, 0, 0) => Rgba([0xD2, 0xF2, 0xA9, 0xff]),
//             (1, 0, 0, 0) => Rgba([0xB1, 0xC6, 0x91, 0xff]),
//             (10, 9, 0, 0) => Rgba([0xff, 0xA9, 0x00, 0xff]),
//             (5, 0, 0, 0) => Rgba([0xB2, 0x8C, 0x5F, 0xff]),
//             (1, 4, 0, 0) => Rgba([0xB1, 0xC6, 0x91, 0xff]),
//             (1, 5, 0, 0) => Rgba([0xff, 0x15, 0x00, 0xff]),
//             (5, 1, 0, 0) => Rgba([0xff, 0x51, 0x00, 0xff]),
//             (10, 5, 1, 0) => Rgba([0xff, 0xa5, 0x10, 0xff]),
//             (10, 1, 5, 0) => Rgba([0xff, 0xa1, 0x50, 0xff]),
//             (10, 1, 0, 0) => Rgba([0xff, 0xa1, 0x00, 0xff]),
//             (10, 5, 0, 0) => Rgba([0xff, 0xa5, 0x00, 0xff]),
//             (10, 9, 1, 0) => Rgba([0xff, 0xa9, 0x10, 0xff]),
//             (10, 1, 9, 0) => Rgba([0xff, 0xa1, 0x90, 0xff]),
//             (4, 1, 0, 0) => Rgba([0xE1, 0xF1, 0xC1, 0xff]),
//             (4, 0, 0, 0) => Rgba([0xE1, 0xF1, 0xC1, 0xff]),
//             (1, 2, 0, 0) => Rgba([0x84, 0x8B, 0x79, 0xff]),
//             (2, 1, 0, 0) => Rgba([0x84, 0x8B, 0x79, 0xff]),
//             (2, 0, 0, 0) => Rgba([0x84, 0x8B, 0x79, 0xff]),
//             (7, 0, 0, 0) => Rgba([0xE8, 0xE1, 0xCD, 0xff]),
//             (7, 1, 0, 0) => Rgba([0xE8, 0xE1, 0xCD, 0xff]),
//             (10, 1, 4, 0) => Rgba([0xff, 0xa1, 0x40, 0xff]),
//             (10, 4, 0, 0) => Rgba([0xff, 0xa4, 0x00, 0xff]),
//             (10, 4, 1, 0) => Rgba([0xff, 0xa4, 0x10, 0xff]),
//             (10, 2, 0, 0) => Rgba([0xff, 0xa2, 0x00, 0xff]),
//             _ => Rgba([0x00, 0x00, 0x00, 0xff]),
//         };

//         // println!("color r{} g{} b{} a{}", r, g, b, a);
//         // dbg!("x", x, ch_x * 32 + x);
//         // dbg!("y", y, ch_y * 32 + y);

//         let width = full_img.width();
//         let height = full_img.height();

//         let mut pos_x = ch_x * 32 + x;
//         let mut pos_y = ch_y * 32 + y;

//         if width == pos_x {
//             pos_x = width - 1;
//         }

//         if height ==  pos_y {
//             pos_y = height - 1;
//         }


//         full_img.put_pixel(pos_x, pos_y, color);

//         // println!("x:{x} y:{y}");
//         img.put_pixel(x, y, color);
//         x += 1;
//         if x >= 32 {
//             x = 0;
//             y += 1;
//         }

//     }

//     // image::imageops::replace(full_img, &img, ch_x * 32, ch_y * 32);
// }

// fn render_land(data: &[u8], full_img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ch_coords: &[i32]) {
//     let mut y = 0;
//     let mut x = 0;
//     // let mut img = ImageBuffer::new(32, 32);
//     let (ch_x, ch_y) = (ch_coords[0] as u32, ch_coords[1] as u32);

//     for chunk in data.chunks(2) {
//         let r = chunk[0];
//         let g = chunk[1];


//         // merge color with pixel
//         let color = hsv_to_rgb(0.0, 0.0, 1.0 * (r as f32 / 128.0));
//         let color = Rgba([color.0 as u8, color.1 as u8, color.2 as u8, 128]);

//         full_img.blend_pixel(ch_x * 32 + x, ch_y * 32 + y, color);
//         x += 1;
//         if x >= 32 {
//             x = 0;
//             y += 1;
//         }
//     }

//     // image::imageops::replace(&mut img, full_img, ch_x * 32, ch_y * 32);
// }

// fn render_height(data: &[u8], full_img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ch_coords: &[i32]) {
//     let mut y = 0;
//     let mut x = 0;
//     // let mut img = ImageBuffer::new(32, 32);
//     let (ch_x, ch_y) = (ch_coords[0] as u32, ch_coords[1] as u32);

//     for chunk in data.chunks(4) {
//         let r = chunk[0];
//         let g = chunk[1];
//         let b = chunk[2];
//         let a = chunk[3];

//         // full_img.put_pixel(ch_x * 32 + x, ch_y * 32 + y, Rgba([r, g, b, 128]));
//         full_img.blend_pixel(ch_x * 32 + x, ch_y * 32 + y, Rgba([r, g, b, 128]));

//         x += 1;
//         if x >= 32 {
//             x = 0;
//             y += 1;
//         }
//     }

//     // image::imageops::replace(full_img, &img, ch_x * 32, ch_y * 32);
// }

// fn render_water(data: &[u8], full_img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ch_coords: &[i32]) {
//     let mut y = 0;
//     let mut x = 0;
//     // let mut img = ImageBuffer::new(32, 32);
//     let (ch_x, ch_y) = (ch_coords[0] as u32, ch_coords[1] as u32);

//     for chunk in data.chunks(2) {
//         let r = chunk[0];
//         let g = chunk[1];

//         if r == 0 {
//         } else {
//             // render blue color
//             let color = Rgba([0, 0, 255, 255]);
//             full_img.put_pixel(ch_x * 32 + x, ch_y * 32 + y, color);
//         };

//         x += 1;
//         if x >= 32 {
//             x = 0;
//             y += 1;
//         }
//     }

//     // image::imageops::replace(full_img, &img, ch_x * 32, ch_y * 32);
// }

// fn render_strange(data: &[u8], full_img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ch_coords: &[i32]) {
//     let mut y = 0;
//     let mut x = 0;
//     // let mut img = ImageBuffer::new(32, 32);
//     let (ch_x, ch_y) = (ch_coords[0] as u32, ch_coords[1] as u32);

//     for &r in data {
//         full_img.put_pixel(ch_x * 32 + x, ch_y * 32 + y, Rgba([r, 0, 0, r]));
//         x += 1;
//         if x >= 32 {
//             x = 0;
//             y += 1;
//         }
//     }

//     // image::imageops::replace(full_img, &img, ch_x * 32, ch_y * 32);
// }

// fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
//     let i = (h * 6.0).floor();
//     let f = h * 6.0 - i;
//     let p = v * (1.0 - s);
//     let q = v * (1.0 - f * s);
//     let t = v * (1.0 - (1.0 - f) * s);
//     match i as i32 % 6 {
//         0 => (v, t, p),
//         1 => (q, v, p),
//         2 => (p, v, t),
//         3 => (p, q, v),
//         4 => (t, p, v),
//         _ => (v, p, q),
//     }
// }

// fn blend_colors(color1: &Rgba<u8>, color2: &Rgba<u8>) -> Rgba<u8> {
//     let r = (color1[0] as u16 + color2[0] as u16) / 2;
//     let g = (color1[1] as u16 + color2[1] as u16) / 2;
//     let b = (color1[2] as u16 + color2[2] as u16) / 2;
//     let a = (color1[3] as u16 + color2[3] as u16) / 2;
//     Rgba([r as u8, g as u8, b as u8, a as u8])
// }

