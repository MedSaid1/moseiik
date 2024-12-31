#!/bin/bash

# Vérifier si le script est exécuté avec les privilèges root
if [ "$EUID" -ne 0 ]; then
    echo "Veuillez exécuter ce script avec les privilèges root."
    exit 1
fi

# Étape 1 : Installer QEMU pour la prise en charge multi-architectures
echo "Installation de QEMU pour la prise en charge multi-architectures..."
docker run --rm --privileged multiarch/qemu-user-static --reset -p yes
if [ $? -ne 0 ]; then
    echo "Échec de l'installation de QEMU."
    exit 1
fi
echo "QEMU installé avec succès."

# Étape 2 : Configurer Docker Buildx pour la construction multi-architectures
echo "Configuration de Docker Buildx..."
docker buildx create --use --name multiarch-builder || echo "Builder déjà configuré."
docker buildx inspect multiarch-builder --bootstrap
if [ $? -ne 0 ]; then
    echo "Échec de la configuration de Docker Buildx."
    exit 1
fi
echo "Docker Buildx configuré avec succès."

# Étape 3 : Créer une image pour ARM64
echo "Création de l'image Docker pour ARM64..."
docker buildx build --platform linux/arm64 -t moseiik-tests-arm --load .
if [ $? -ne 0 ]; then
    echo "Échec de la création de l'image Docker pour ARM64."
    exit 1
fi
echo "Image Docker pour ARM64 créée avec succès."

# Étape 4 : Créer une image pour AMD64
echo "Création de l'image Docker pour AMD64..."
docker buildx build --platform linux/amd64 -t moseiik-tests-amd --load .
if [ $? -ne 0 ]; then
    echo "Échec de la création de l'image Docker pour AMD64."
    exit 1
fi
echo "Image Docker pour AMD64 créée avec succès."

# Étape 5 : Exécuter les tests pour ARM64
echo "Exécution des tests pour ARM64..."
docker run --rm --platform linux/arm64 moseiik-tests-arm
if [ $? -ne 0 ]; then
    echo "Tests échoués pour ARM64."
else
    echo "Tests réussis pour ARM64."
fi

# Étape 6 : Exécuter les tests pour AMD64
echo "Exécution des tests pour AMD64..."
docker run --rm --platform linux/amd64 moseiik-tests-amd
if [ $? -ne 0 ]; then
    echo "Tests échoués pour AMD64."
else
    echo "Tests réussis pour AMD64."
fi

# Fin du script
echo "Processus terminé."

