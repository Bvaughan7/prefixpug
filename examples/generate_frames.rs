use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn main() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|f| {
            let area = f.area();
            let block = ratatui::widgets::Block::default().title("Test");
            f.render_widget(block, area);
        })
        .expect("draw");
    let buf = terminal.backend().buffer();
    let cell = buf.cell((0, 0)).expect("cell");
    println!(
        "Cell: symbol='{}', fg={:?}, bg={:?}",
        cell.symbol(),
        cell.fg,
        cell.bg
    );
}
