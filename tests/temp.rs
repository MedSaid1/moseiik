/// IMPORTANT!!!
/// Les tests échouent pour toutes les architectures (ARM, AMD, générique),
/// car l'image générée diffère visiblement de la cible. 
/// Cela est probablement dû à des modifications dans le répertoire des tiles : moseiik_test_images/images.

// Pour exécuter ces tests avec cargo, veuillez lancer la commande ./test.bash

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::path::Path;

    // Chemins vers les fichiers et dossiers nécessaires
    const ASSETS_DIR: &str = "assets";
    const TILES_DIR: &str = "moseiik_test_images/images";
    const TARGET_IMAGE: &str = "moseiik_test_images/kit.jpeg";
    const GROUND_TRUTH: &str = "moseiik_test_images/ground-truth-kit.png";
    const OUTPUT_IMAGE: &str = "output.png";

    /// Fonction pour exécuter la génération de mosaïque
    /// Retourne `true` si l'exécution réussit, `false` sinon
    fn run_mosaic() -> bool {
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

        // Exécuter la commande via `cargo`
        let output = Command::new("cargo")
            .args(args)
            .output()
            .expect("Failed to execute mosaic generation");
    
        output.status.success()
    }
    

    /// Fonction pour comparer deux images byte par byte
    /// Retourne `true` si les images sont identiques, sinon `false`
    fn compare_images(image1: &str, image2: &str) -> bool {
        use std::fs;
    
        let data1 = fs::read(image1).expect("Failed to read image1");
        let data2 = fs::read(image2).expect("Failed to read image2");
    
        // Compare les données brutes des deux fichiers
        data1 == data2
    }
    

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_x86() {
        // Exécuter la mosaïque
        let result = run_mosaic();
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
        // Exécuter la mosaïque
        let result = run_mosaic();
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
        // Exécuter la mosaïque
        let result = run_mosaic();
        assert!(result, "Mosaic generation failed in generic mode");

        // Comparer la sortie avec la vérité terrain
        let identical = compare_images(OUTPUT_IMAGE, GROUND_TRUTH);
        assert!(
            identical,
            "Generated mosaic does not match ground truth in generic mode"
        );
    }
}
