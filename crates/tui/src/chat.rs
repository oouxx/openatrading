use std::sync::Arc;
use std::sync::Mutex;

use futures::StreamExt;
use pi_tui::keybindings::init_keybindings;
use pi_tui::Terminal;
use ratatui::prelude::*;
use ratatui::widgets::*;

fn process_user_input(
    text: String,
    llm_messages: &Arc<Mutex<Vec<serde_json::Value>>>,
    display_messages: &Arc<Mutex<Vec<(String, String)>>>,
    streaming: &Arc<Mutex<bool>>,
    stream_buffer: &Arc<Mutex<String>>,
    llm: openatrading_core::llm::LlmClient,
) {
    display_messages
        .lock()
        .unwrap()
        .push(("user".into(), text.clone()));
    llm_messages
        .lock()
        .unwrap()
        .push(serde_json::json!({"role": "user", "content": text}));
    *streaming.lock().unwrap() = true;
    *stream_buffer.lock().unwrap() = String::new();

    let msgs = llm_messages.clone();
    let disp = display_messages.clone();
    let strm = streaming.clone();
    let sb = stream_buffer.clone();

    tokio::spawn(async move {
        let msgs_json = {
            let locked = msgs.lock().unwrap();
            serde_json::to_string(&*locked).unwrap_or_default()
        };

        match llm.chat_stream_with_messages(&msgs_json).await {
            Ok(mut stream) => {
                let mut response = String::new();
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(delta) => {
                            response.push_str(&delta);
                            *sb.lock().unwrap() = response.clone();
                        },
                        Err(e) => {
                            response.push_str(&format!("\n[Error: {}]", e));
                            *sb.lock().unwrap() = response.clone();
                            break;
                        },
                    }
                }
                msgs.lock()
                    .unwrap()
                    .push(serde_json::json!({"role": "assistant", "content": response}));
                disp.lock().unwrap().push(("assistant".into(), response));
                *sb.lock().unwrap() = String::new();
                *strm.lock().unwrap() = false;
            },
            Err(e) => {
                let err = format!("Error: {}", e);
                disp.lock().unwrap().push(("assistant".into(), err));
                *sb.lock().unwrap() = String::new();
                *strm.lock().unwrap() = false;
            },
        }
    });
}

pub async fn run_chat() -> anyhow::Result<()> {
    init_keybindings(None);

    let llm = openatrading_core::llm::LlmClient::from_config(None, None);

    let mut terminal = Terminal::new()?;
    let (mut input_rx, shutdown_guard) = terminal.start()?;

    let display_messages: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_messages: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(vec![
        serde_json::json!({"role": "system", "content": "You are a helpful AI trading assistant. You can help with market analysis and trading questions."}),
    ]));
    let input: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let streaming: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let stream_buffer: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let should_quit: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    display_messages.lock().unwrap().push((
        "system".into(),
        "AI Chat — Type your message and press Enter. Press Esc to quit.".into(),
    ));

    let ratatui_term = terminal.ratatui_terminal();

    let mut render_ticker = tokio::time::interval(tokio::time::Duration::from_millis(33));

    loop {
        if *should_quit.lock().unwrap() {
            break;
        }

        tokio::select! {
            _ = render_ticker.tick() => {}
            Some(data) = input_rx.recv() => {
                let strm = *streaming.lock().unwrap();

                if !strm {
                    match data.as_str() {
                        "\r" | "\n" => {
                            let text = {
                                let mut inp = input.lock().unwrap();
                                std::mem::take(&mut *inp)
                            };
                            if !text.is_empty() {
                                process_user_input(
                                    text,
                                    &llm_messages,
                                    &display_messages,
                                    &streaming,
                                    &stream_buffer,
                                    llm.clone(),
                                );
                            }
                        }
                        "\x1b" => {
                            *should_quit.lock().unwrap() = true;
                        }
                        "\x7f" => {
                            input.lock().unwrap().pop();
                        }
                        c if !c.is_empty() && c.len() == 1 => {
                            input.lock().unwrap().push_str(c);
                        }
                        _ => {}
                    }
                }
            }
        }

        ratatui_term.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);
            let (chat_area, input_area) = (chunks[0], chunks[1]);

            let msgs = display_messages.lock().unwrap();
            let sb = stream_buffer.lock().unwrap();
            let strm = *streaming.lock().unwrap();

            let mut display = String::new();
            for (role, content) in msgs.iter() {
                let prefix = match role.as_str() {
                    "user" => "\n[You]\n",
                    "system" => "",
                    _ => "\n[AI]\n",
                };
                display.push_str(prefix);
                display.push_str(content);
                display.push('\n');
            }
            if strm && !sb.is_empty() {
                display.push_str("\n[AI]\n");
                display.push_str(&sb);
            }

            let chat = Paragraph::new(display)
                .wrap(Wrap { trim: false })
                .style(Style::default());
            frame.render_widget(chat, chat_area);

            let inp = input.lock().unwrap();
            let input_suffix = if strm { " (streaming...)" } else { "" };
            let input_style = if strm {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let input_widget =
                Paragraph::new(format!("❯ {}{}", *inp, input_suffix)).style(input_style);
            frame.render_widget(input_widget, input_area);

            if !strm {
                let cursor_x = 2u16
                    .saturating_add(inp.len() as u16)
                    .min(area.width.saturating_sub(1));
                frame.set_cursor_position((cursor_x, area.height.saturating_sub(1)));
            }
            drop(inp);
            drop(sb);
            drop(msgs);
        })?;
    }

    shutdown_guard.shutdown();
    Ok(())
}
