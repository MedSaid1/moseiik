# Veuillez utiliser le script `docker.sh` pour simplifier le processus. commande: sudo ./docker.sh
# Ce script :
# - Installe QEMU
# - Configure Docker Buildx pour les builds multi-architectures
# - Construit deux images Docker (ARM64 et AMD64) localement (--load)
# - Exécute les tests pour chaque architecture


# Étape 1 : Image de base compatible multi-architectures
FROM rust:latest

# Étape 2 : Installer des dépendances supplémentaires
RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Étape 3 : Définir le répertoire de travail
WORKDIR /app

# Étape 4 : Copier les sources dans le conteneur
COPY . .

# Étape 5 : Installer les dépendances et compiler le projet
RUN cargo build --release

# Étape 6 : Configurer l'entrypoint pour les tests
ENTRYPOINT [ "cargo", "test", "--release", "--" ]
