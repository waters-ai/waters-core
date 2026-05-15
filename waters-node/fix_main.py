import sys

with open(sys.argv[1]) as f:
    content = f.read()

# Find the corrupted _ => arm
marker = '            _ => {'
idx = content.find(marker)
if idx < 0:
    print("ERROR: _ => not found")
    sys.exit(1)

# Count braces to find where this arm ends
depth = 0
for i in range(idx, len(content)):
    if content[i] == '{':
        depth += 1
    elif content[i] == '}':
        depth -= 1
        if depth == 0:
            end = i + 1
            break
else:
    print("ERROR: unclosed _ => arm")
    sys.exit(1)

# Get the indentation from the surrounding code
# Find the "chat " arm before this
prev_match = content.rfind('text if text.to_lowercase().starts_with("chat ")', idx - 200, idx)
if prev_match < 0:
    prev_match = content.rfind('text if text.to_lowercase().starts_with(', idx - 200, idx)

# Build clean replacement
clean = """            _ => {
                if let Some(ref l) = llm_client {
                    session_mgr.add_message("user", cmd);
                    let mut ci = chat::ChatInterface::new(l.clone(), &session_mgr);
                    match ci.process(cmd).await {
                        Ok(r) => { println!("{}", r); session_mgr.add_message("assistant", &r); }
                        Err(e) => {
                            let response = convo.handle(cmd);
                            if response == "exit" {
                                println!("Shutting down...");
                                session_mgr.save()?;
                                convo.save(&convo_path);
                                node.save_state(&state_path)?;
                                break;
                            }
                            println!("{}", response);
                        }
                    }
                } else {
                    if let Some(reply) = handle_convo(&mut convo, &convo_path, cmd, &task_mgr, &agent_mgr, &group_mgr, &gossip, &skill_reg, &bridge_reg, &mut node, &state_path, &mut session_mgr).await {
                        println!("{}", reply);
                    } else { break; }
                }
            }"""

new_content = content[:idx] + clean + content[end:]
with open(sys.argv[1], 'w') as f:
    f.write(new_content)
print("Fixed")
