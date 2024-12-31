# Étape 1 : Image de base compatible multi-architectures
FROM rust:latest

# Étape 2 : Installer des dépendances supplémentaires (si nécessaire)
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
