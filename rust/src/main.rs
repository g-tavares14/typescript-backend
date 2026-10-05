// Ponto de entrada do servidor (o equivalente ao src/server.ts).
// `#[tokio::main]` é uma macro: transforma a `main` assíncrona numa `main` normal que liga o runtime do tokio
// (o "event loop" que, no Node, já vem pronto) e roda a função dentro dele.
#[tokio::main]
async fn main() {
    println!("meu-backend (Rust): esqueleto");
}
