use clap::Parser;
use image::{
    imageops::{resize, FilterType::Nearest},
    GenericImage, GenericImageView, ImageReader, RgbImage,
};
use std::time::Instant;
use std::{
    error::Error,
    fs,
    ops::Deref,
    sync::{Arc, Mutex},
};
use threadpool::ThreadPool;
use threadpool_scope::scope_with;

#[derive(Debug, Parser)]
struct Size {
    width: u32,
    height: u32,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Options {
    /// Location of the target image
    #[arg(short, long)]
    pub image: String,

    /// Saved result location
    #[arg(short, long, default_value_t=String::from("out.png"))]
    pub output: String,

    /// Location of the tiles
    #[arg(short, long)]
    pub tiles: String,

    /// Scaling factor of the image
    #[arg(long, default_value_t = 1)]
    pub scaling: u32,

    /// Size of the tiles
    #[arg(long, default_value_t = 5)]
    pub tile_size: u32,

    /// Remove used tile
    #[arg(short, long)]
    pub remove_used: bool,

    #[arg(short, long)]
    pub verbose: bool,

    /// Use SIMD when available
    #[arg(short, long)]
    pub simd: bool,

    /// Specify number of threads to use, leave blank for default
    #[arg(short, long, default_value_t = 1)]
    pub num_thread: usize,
}

fn count_available_tiles(images_folder: &str) -> i32 {
    match fs::read_dir(images_folder) {
        Ok(t) => return t.count() as i32,
        Err(_) => return -1,
    };
}

fn prepare_tiles(
    images_folder: &str,
    tile_size: &Size,
    verbose: bool,
) -> Result<Vec<RgbImage>, Box<dyn Error>> {
    let nb_tiles: usize = count_available_tiles(images_folder) as usize;

    let mut tile_names: Vec<_> = fs::read_dir(images_folder)
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    tile_names.sort_by_key(|dir| dir.path());

    // Declare a vector in a nb_tiles wide memory segment
    let tiles = Arc::new(Mutex::new(Vec::with_capacity(nb_tiles) as Vec<RgbImage>));
    // Actually allocate memory
    tiles.lock().unwrap().resize(nb_tiles, RgbImage::new(0, 0));

    let now = Instant::now();
    let pool = ThreadPool::new(num_cpus::get());
    let tile_width = tile_size.width;
    let tile_height = tile_size.height;

    // for image_path in image_paths
    for (index, image_path) in (0..=nb_tiles - 1).zip(tile_names) {
        let tiles = Arc::clone(&tiles);
        pool.execute(move || {
            let tile_result = || -> Result<RgbImage, Box<dyn Error>> {
                Ok(ImageReader::open(image_path.path())?.decode()?.into_rgb8())
            };

            let tile = match tile_result() {
                Ok(t) => t,
                Err(_) => return,
            };

            let tile = resize(&tile, tile_width, tile_height, Nearest);
            tiles.lock().unwrap()[index] = tile;
        });
    }
    pool.join();

    println!(
        "\n{} elements in {} seconds",
        tiles.lock().unwrap().len(),
        now.elapsed().as_millis() as f32 / 1000.0
    );

    if verbose {
        println!("");
    }
    let res = tiles.lock().unwrap().deref().to_owned();
    return Ok(res);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2", enable = "avx")]
unsafe fn l1_x86_avx2(im1: &RgbImage, im2: &RgbImage) -> i32 {
    // Suboptimal performance due to the use of _mm256_loadu_si256.
    use std::arch::x86_64::{
        __m256i,
        _mm256_extract_epi16, //AVX2
        _mm256_load_si256,    //AVX
        _mm256_loadu_si256,   //AVX
        _mm256_sad_epu8,      //AVX2
    };

    let stride = std::mem::size_of::<__m256i>();

    let tile_size = (im1.width() * im1.height()) as usize;
    let nb_sub_pixel = tile_size * 3;

    let im1 = im1.as_raw();
    let im2 = im2.as_raw();

    let mut result: i32 = 0;

    for i in (0..nb_sub_pixel - stride).step_by(stride) {
        // Get pointer to data
        let p_im1: *const __m256i =
            std::mem::transmute::<*const u8, *const __m256i>(std::ptr::addr_of!(im1[i as usize]));
        let p_im2: *const __m256i =
            std::mem::transmute::<*const u8, *const __m256i>(std::ptr::addr_of!(im2[i as usize]));

        // Load data to ymm
        let ymm_p1 = _mm256_loadu_si256(p_im1);
        let ymm_p2 = _mm256_load_si256(p_im2);

        // Do abs(a-b) and horizontal add, results are stored in lower 16 bits of each 64 bits groups
        let ymm_sub_abs = _mm256_sad_epu8(ymm_p1, ymm_p2);

        let res_0 = _mm256_extract_epi16(ymm_sub_abs, 0);
        let res_1 = _mm256_extract_epi16(ymm_sub_abs, 4);
        let res_2 = _mm256_extract_epi16(ymm_sub_abs, 8);
        let res_3 = _mm256_extract_epi16(ymm_sub_abs, 12);

        result += res_0 + res_1 + res_2 + res_3;
    }

    // now do the remainder manually
    let remainder = nb_sub_pixel % stride;
    for i in nb_sub_pixel - remainder..nb_sub_pixel {
        let p1: u8 = im1[i as usize];
        let p2: u8 = im2[i as usize];

        result += i32::abs((p1 as i32) - (p2 as i32));
    }

    return result;
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn l1_x86_sse2(im1: &RgbImage, im2: &RgbImage) -> i32 {
    // Only works if data is 16 bytes-aligned, which should be the case.
    // In case of crash due to unaligned data, swap _mm_load_si128 for _mm_loadu_si128.
    use std::arch::x86_64::{
        __m128i,
        _mm_extract_epi16, //SSE2
        _mm_load_si128,    //SSE2
        _mm_sad_epu8,      //SSE2
    };

    let stride = std::mem::size_of::<__m128i>();

    let tile_size = (im1.width() * im1.height()) as usize;
    let nb_sub_pixel = tile_size * 3;

    let im1 = im1.as_raw();
    let im2 = im2.as_raw();

    let mut result: i32 = 0;

    for i in (0..nb_sub_pixel - stride).step_by(stride) {
        // Get pointer to data
        let p_im1: *const __m128i =
            std::mem::transmute::<*const u8, *const __m128i>(std::ptr::addr_of!(im1[i as usize]));
        let p_im2: *const __m128i =
            std::mem::transmute::<*const u8, *const __m128i>(std::ptr::addr_of!(im2[i as usize]));

        // Load data to xmm
        let xmm_p1 = _mm_load_si128(p_im1);
        let xmm_p2 = _mm_load_si128(p_im2);

        // Do abs(a-b) and horizontal add, results are stored in lower 16 bits of each 64 bits groups
        let xmm_sub_abs = _mm_sad_epu8(xmm_p1, xmm_p2);

        let res_0 = _mm_extract_epi16(xmm_sub_abs, 0);
        let res_1 = _mm_extract_epi16(xmm_sub_abs, 4);

        result += res_0 + res_1;
    }

    // now do the remainder manually
    let remainder = nb_sub_pixel % stride;
    for i in nb_sub_pixel - remainder..nb_sub_pixel {
        let p1: u8 = im1[i];
        let p2: u8 = im2[i];

        result += i32::abs((p1 as i32) - (p2 as i32));
    }

    return result;
}

fn l1_generic(im1: &RgbImage, im2: &RgbImage) -> i32 {
    im1.iter()
        .zip(im2.iter())
        .fold(0, |res, (a, b)| res + i32::abs((*a as i32) - (*b as i32)))
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn l1_neon(im1: &RgbImage, im2: &RgbImage) -> i32 {
    use std::arch::aarch64::uint8x16_t;
    use std::arch::aarch64::vabdq_u8; // Absolute subtract
    use std::arch::aarch64::vaddlvq_u8; // horizontal add
    use std::arch::aarch64::vld1q_u8; // Load instruction

    let stride = std::mem::size_of::<uint8x16_t>();

    let tile_size = (im1.width() * im1.height()) as usize;
    let nb_sub_pixel = tile_size * 3;

    let im1 = im1.as_raw();
    let im2 = im2.as_raw();

    let mut result: i32 = 0;

    for i in (0..nb_sub_pixel - stride).step_by(stride as usize) {
        // get pointer to data
        let p_im1: *const u8 = std::ptr::addr_of!(im1[i as usize]);
        let p_im2: *const u8 = std::ptr::addr_of!(im2[i as usize]);

        // load data to xmm
        let xmm1: uint8x16_t = vld1q_u8(p_im1);
        let xmm2: uint8x16_t = vld1q_u8(p_im2);

        // get absolute difference
        let xmm_abs_diff: uint8x16_t = vabdq_u8(xmm1, xmm2);

        // reduce with horizontal add
        result += vaddlvq_u8(xmm_abs_diff) as i32;
    }

    // now do the remainder manually
    let remainder = nb_sub_pixel % stride;
    for i in nb_sub_pixel - remainder..nb_sub_pixel {
        let p1: u8 = im1[i as usize];
        let p2: u8 = im2[i as usize];

        result += i32::abs((p1 as i32) - (p2 as i32));
    }

    return result;
}

fn l1(im1: &RgbImage, im2: &RgbImage, simd_flag: bool, verbose: bool) -> i32 {
    return unsafe { get_optimal_l1(simd_flag, verbose)(im1, im2) };
}

unsafe fn get_optimal_l1(simd_flag: bool, verbose: bool) -> unsafe fn(&RgbImage, &RgbImage) -> i32 {
    static mut FN_POINTER: unsafe fn(&RgbImage, &RgbImage) -> i32 = l1_generic;

    static INIT: std::sync::Once = std::sync::Once::new();

    INIT.call_once(|| {
        if simd_flag {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            {
                if is_x86_feature_detected!("avx2") {
                    if verbose {
                        println!("{}[2K\rUsing AVX2 SIMD.", 27 as char);
                    }
                    FN_POINTER = l1_x86_avx2;
                } else if is_x86_feature_detected!("sse2") {
                    if verbose {
                        println!("{}[2K\rUsing SSE2 SIMD.", 27 as char);
                    }
                    FN_POINTER = l1_x86_sse2;
                } else {
                    if verbose {
                        println!("{}[2K\rNot using SIMD.", 27 as char);
                    }
                }
            }
            #[cfg(target_arch = "aarch64")]
            {
                use std::arch::is_aarch64_feature_detected;
                if is_aarch64_feature_detected!("neon") {
                    if verbose {
                        println!("{}[2K\rUsing NEON SIMD.", 27 as char);
                    }
                    FN_POINTER = l1_neon;
                }
            }
        }
    });

    return FN_POINTER;
}

fn prepare_target(
    image_path: &str,
    scale: u32,
    tile_size: &Size,
) -> Result<RgbImage, Box<dyn Error>> {
    let target = ImageReader::open(image_path)?.decode()?.into_rgb8();
    let target = resize(
        &target,
        target.width() * scale,
        target.height() * scale,
        Nearest,
    );
    Ok(target
        .view(
            0,
            0,
            target.width() - target.width() % tile_size.width,
            target.height() - target.height() % tile_size.height,
        )
        .to_image())
}

pub fn compute_mosaic(args: Options) {
    let tile_size = Size {
        width: args.tile_size,
        height: args.tile_size,
    };

    let (target_size, target) = match prepare_target(&args.image, args.scaling, &tile_size) {
        Ok(t) => (
            Size {
                width: t.width(),
                height: t.height(),
            },
            Arc::new(Mutex::new(t)),
        ),
        Err(e) => panic!("Error opening {}. {}", args.image, e),
    };

    let nb_available_tiles = count_available_tiles(&args.tiles);
    let nb_required_tiles: i32 =
        ((target_size.width / tile_size.width) * (target_size.height / tile_size.height)) as i32;
    if args.remove_used && nb_required_tiles > nb_available_tiles {
        panic!(
            "{} tiles required, found {}.",
            nb_required_tiles, nb_available_tiles
        )
    }

    let tiles = &prepare_tiles(&args.tiles, &tile_size, args.verbose).unwrap();
    if args.verbose {
        println!("w: {}, h: {}", target_size.width, target_size.height);
    }

    let now = Instant::now();
    let pool = ThreadPool::new(args.num_thread);
    scope_with(&pool, |scope| {
        for w in 0..target_size.width / tile_size.width {
            let target = Arc::clone(&target);
            scope.execute(move || {
                for h in 0..target_size.height / tile_size.height {
                    if args.verbose {
                        print!(
                            "\rBuilding image: {} / {} : {} / {}",
                            w,
                            target_size.width / tile_size.width,
                            h,
                            target_size.height / tile_size.height
                        );
                    }
                    let mut best_tile = 0;
                    let mut min_error = i32::MAX;
                    let target_tile = &(target
                        .lock()
                        .unwrap()
                        .view(
                            tile_size.width * w,
                            tile_size.height * h,
                            tile_size.width,
                            tile_size.height,
                        )
                        .to_image());

                    for (i, tile) in tiles.iter().enumerate() {
                        let error = l1(tile, &target_tile, args.simd, args.verbose);

                        if error < min_error {
                            min_error = error;
                            best_tile = i;
                        }
                    }

                    target
                        .lock()
                        .unwrap()
                        .copy_from(&tiles[best_tile], w * tile_size.width, h * tile_size.height)
                        .unwrap();
                }
            });
        }
    });
    println!("\n{} seconds", now.elapsed().as_millis() as f32 / 1000.0);

    target.lock().unwrap().save(args.output).unwrap();
}

fn main() {
    let args = Options::parse();
    compute_mosaic(args);
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{RgbImage, Rgb};

    //Pour exécuter ces tests avec cargo, veuillez lancer la commande ./test.bash

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn unit_test_x86() {
        // Création de deux images 7x7 avec des valeurs RGB différentes pour tester l'implémentation SIMD sur x86/x86_64
        // Création de deux images 7x7 avec des valeurs RGB différentes pour tester l'implémentation SIMD sur x86/x86_64
        let img1 = RgbImage::from_fn(7, 7, |x, y| image::Rgb([x as u8, y as u8, 0]));
        let img2 = RgbImage::from_fn(7, 7, |x, y| image::Rgb([(x + 1) as u8, (y + 1) as u8, 0]));

        // Calcul manuel du résultat attendu (somme des différences absolues entre les pixels)
        // Calcul manuel du résultat attendu (somme des différences absolues entre les pixels)
        let mut expected_result: i32 = 0;
        for y in 0..7 {
            for x in 0..7 {
                let pixel1 = [x as u8, y as u8, 0];
                let pixel2 = [(x + 1) as u8, (y + 1) as u8, 0];

                for channel in 0..3 {
                    expected_result += (pixel1[channel] as i32 - pixel2[channel] as i32).abs();
                }
            }
        }

        // Appel de la fonction SIMD optimisée
        // Appel de la fonction SIMD optimisée
        unsafe {
            let simd_result = l1_x86_sse2(&img1, &img2);

            // Vérification que le résultat SIMD correspond au résultat attendu
            // Vérification que le résultat SIMD correspond au résultat attendu
            assert_eq!(
                simd_result, expected_result,
                "L1 SIMD result ({}) does not match the expected result ({})",
                simd_result, expected_result
            );
        }
    }


    #[test]
    #[cfg(target_arch = "aarch64")]
    fn unit_test_aarch64() {
        // Même test que pour x86, mais adapté à ARM (architecture aarch64)
        // Même test que pour x86, mais adapté à ARM (architecture aarch64)
        let img1 = RgbImage::from_fn(7, 7, |x, y| image::Rgb([x as u8, y as u8, 0]));
        let img2 = RgbImage::from_fn(7, 7, |x, y| image::Rgb([(x + 1) as u8, (y + 1) as u8, 0]));

        // Calculer manuellement la distance L1
        let mut expected_result: i32 = 0;
        for y in 0..7 {
            for x in 0..7 {
                let pixel1 = [x as u8, y as u8, 0];
                let pixel2 = [(x + 1) as u8, (y + 1) as u8, 0];

                for channel in 0..3 {
                    expected_result += (pixel1[channel] as i32 - pixel2[channel] as i32).abs();
                }
            }
        }

        unsafe {
            let simd_result = l1_neon(&img1, &img2);

            // Vérification que le résultat SIMD correspond au résultat attendu
            // Vérification que le résultat SIMD correspond au résultat attendu
            assert_eq!(
                simd_result, expected_result,
                "L1 SIMD result ({}) does not match the expected result ({})",
                simd_result, expected_result
            );
        }
    }


    #[test]
    fn unit_test_generic() {
        // Test générique pour vérifier la distance L1 entre deux petites images 2x2
        // Test générique pour vérifier la distance L1 entre deux petites images 2x2
        let mut img1 = RgbImage::new(2, 2);
        let mut img2 = RgbImage::new(2, 2);

        img1.put_pixel(0, 0, Rgb([10, 20, 30]));
        img1.put_pixel(1, 0, Rgb([40, 50, 60]));
        img1.put_pixel(0, 1, Rgb([70, 80, 90]));
        img1.put_pixel(1, 1, Rgb([100, 110, 120]));

        img2.put_pixel(0, 0, Rgb([5, 15, 25]));
        img2.put_pixel(1, 0, Rgb([35, 45, 55]));
        img2.put_pixel(0, 1, Rgb([65, 75, 85]));
        img2.put_pixel(1, 1, Rgb([95, 105, 115]));

        let expected_result = 5 + 5 + 5 + 5 + 5 + 5 + 5 + 5 + 5 + 5 + 5 + 5; // Différences absolues sommées
        let expected_result = 5 + 5 + 5 + 5 + 5 + 5 + 5 + 5 + 5 + 5 + 5 + 5; // Différences absolues sommées
        let result = l1_generic(&img1, &img2);

        // Vérification du résultat générique
        // Vérification du résultat générique
        assert_eq!(result, expected_result, "L1 distance (generic) is incorrect");
    }

    #[test]
    fn unit_test_prepare_target() {
        // Prépare une cible avec une image et vérifie qu'elle est correctement redimensionnée
        // Prépare une cible avec une image et vérifie qu'elle est correctement redimensionnée
        let tile_size = Size { width: 6, height: 6 };
        let image_path = "test_image.png";
        let scale = 2;

        // Créer une image de test
        let mut img = RgbImage::new(10, 10);
        for x in 0..10 {
            for y in 0..10 {
                img.put_pixel(x, y, Rgb([x as u8, y as u8, (x + y) as u8]));
            }
        }
        img.save(image_path).unwrap();

        let prepared_target = prepare_target(image_path, scale, &tile_size).unwrap();

        // Vérifier que la taille correspond à l'échelle
        assert_eq!(prepared_target.width() % tile_size.width,0,"Prepared target width is not a multiple of tile size width");
        assert_eq!(prepared_target.height() % tile_size.height,0,"Prepared target height is not a multiple of tile size height");


        std::fs::remove_file(image_path).unwrap();
    }

    #[test]
    fn unit_test_prepare_tiles() {
        // Prépare des tuiles à partir d'images et vérifie les dimensions
        // Prépare des tuiles à partir d'images et vérifie les dimensions
        let tiles_folder = "test_tiles";
        let tile_size = Size { width: 5, height: 5 };

        // Créer un dossier de test et des images
        std::fs::create_dir_all(tiles_folder).unwrap();
        for i in 0..3 {
            let mut img = RgbImage::new(10, 10);
            for x in 0..10 {
                for y in 0..10 {
                    img.put_pixel(x, y, Rgb([x as u8, y as u8, i as u8]));
                }
            }
            img.save(format!("{}/tile_{}.png", tiles_folder, i)).unwrap();
        }

        let prepared_tiles = prepare_tiles(tiles_folder, &tile_size, false).unwrap();

        // Vérifier que le nombre de tuiles est correct
        assert_eq!(prepared_tiles.len(), 3);

        // Vérifier les dimensions des tuiles
        for tile in prepared_tiles {
            assert_eq!(tile.width(), tile_size.width);
            assert_eq!(tile.height(), tile_size.height);
        }

        // Nettoyer
        std::fs::remove_dir_all(tiles_folder).unwrap();
    }
}
