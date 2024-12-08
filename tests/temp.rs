#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::path::Path;

    /// Chemins vers les fichiers et dossiers nécessaires
    const ASSETS_DIR: &str = "assets";
    const TILES_DIR: &str = "moseiik_test_images/images";
    const TARGET_IMAGE: &str = "moseiik_test_images/kit.jpeg";
    const GROUND_TRUTH: &str = "moseiik_test_images/ground-truth-kit.png";
    const OUTPUT_IMAGE: &str = "output.png";

    fn run_mosaic(simd: bool) -> bool {
        let mut args = vec![
        "run",
        "--release",
        "--",
        "--image",
        TARGET_IMAGE,
        "--tiles",
        TILES_DIR,
        "--output",
        OUTPUT_IMAGE,
        "--tile-size",
        "25",
        ];

        if simd {
            args.push("--simd");
        }

        let output = Command::new("cargo")
            .args(args)
            .output()
            .expect("Failed to execute mosaic generation");
        
        // Afficher la sortie et les erreurs si la commande échoue
        /*if !output.status.success() {
            eprintln!(
                "Error: {}\nStdout: {}\nStderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }*/
    
        output.status.success()
    }
    

    fn compare_images(image1: &str, image2: &str) -> bool {
        use std::fs;
    
        let data1 = fs::read(image1).expect("Failed to read image1");
        let data2 = fs::read(image2).expect("Failed to read image2");
    
        data1 == data2
    }
    

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_x86() {
        // Exécuter la mosaïque avec SIMD activé
        let result = run_mosaic(true);
        assert!(
            result,
            "Mosaic generation failed with SIMD on x86 or x86_64 architecture"
        );

        // Comparer la sortie avec la vérité terrain
        let identical = compare_images(OUTPUT_IMAGE, GROUND_TRUTH);
        assert!(
            identical,
            "Generated mosaic does not match ground truth on x86 or x86_64"
        );
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_aarch64() {
        // Exécuter la mosaïque avec SIMD activé
        let result = run_mosaic(true);
        assert!(
            result,
            "Mosaic generation failed with SIMD on aarch64 architecture"
        );

        // Comparer la sortie avec la vérité terrain
        let identical = compare_images(OUTPUT_IMAGE, GROUND_TRUTH);
        assert!(
            identical,
            "Generated mosaic does not match ground truth on aarch64"
        );
    }

    #[test]
    fn test_generic() {
        // Exécuter la mosaïque sans SIMD
        let result = run_mosaic(false);
        assert!(result, "Mosaic generation failed in generic mode");

        // Comparer la sortie avec la vérité terrain
        let identical = compare_images(OUTPUT_IMAGE, GROUND_TRUTH);
        assert!(
            identical,
            "Generated mosaic does not match ground truth in generic mode"
        );
    }
}
