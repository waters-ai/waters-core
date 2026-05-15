import sys
with open(sys.argv[1]) as f:
    content = f.read()

# Replace convo.handle calls
old = 'let response = convo.handle(slash_arg);\n                        if response == "exit" { break; }\n                        println!("{}", response);'
new = 'process_convo(&mut convo, &convo_path, slash_arg, &task_mgr, &agent_mgr, &group_mgr, &gossip, &skill_reg, &bridge_reg, &mut node, &state_path, &mut session_mgr).await;'
content = content.replace(old, new)

old = 'let response = convo.handle(cmd);\n                            if response == "exit" {\n                                println!("Shutting down...");\n                                session_mgr.save()?;\n                                convo.save(&convo_path);\n                                node.save_state(&state_path)?;\n                                break;\n                            }\n                            println!("{}", response);\n                        }\n                    }\n                } else {\n                    // Convo handles everything when no LLM\n                    let response = convo.handle(cmd);\n                    if response == "exit" {\n                        println!("Shutting down...");\n                        session_mgr.save()?;\n                        convo.save(&convo_path);\n                        node.save_state(&state_path)?;\n                        break;\n                    }\n                    println!("{}", response);\n                }'
new = 'process_convo(&mut convo, &convo_path, cmd, &task_mgr, &agent_mgr, &group_mgr, &gossip, &skill_reg, &bridge_reg, &mut node, &state_path, &mut session_mgr).await;\n                        }\n                    }\n                } else {\n                    process_convo(&mut convo, &convo_path, cmd, &task_mgr, &agent_mgr, &group_mgr, &gossip, &skill_reg, &bridge_reg, &mut node, &state_path, &mut session_mgr).await;\n                }'
content = content.replace(old, new)

with open(sys.argv[1], 'w') as f:
    f.write(content)
print("Patched")
