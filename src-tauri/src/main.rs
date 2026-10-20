// Sem console no build de release: o app vive na bandeja.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // RF-56: lançados como administrador pela instância comum, esperamos ela
    // sair antes de inicializar — o plugin de instância única não nos mataria
    // só porque a antiga ainda está fechando o tor.
    let args: Vec<String> = std::env::args().collect();
    if let Some(pid) = args
        .windows(2)
        .find(|pair| pair[0] == nika_lib::AFTER_PID_FLAG)
        .and_then(|pair| pair[1].parse::<u32>().ok())
    {
        nika_lib::wait_for_pid(pid);
    }

    nika_lib::run();
}
