content = open("/home/constructor/projects/kapelka/src/pages/Home.tsx").read()

old = 'function WatersNodeDownload({ walletConnected }: { walletConnected: boolean }) {'
idx = content.find(old)
if idx == -1:
    print("Old function not found")
    exit(1)

# Find the end - iterate braces
brace_count = 0
end_idx = idx
for i in range(idx, len(content)):
    if content[i] == '{':
        brace_count += 1
    elif content[i] == '}':
        brace_count -= 1
        if brace_count == 0:
            end_idx = i + 1
            break

new_func = '''function WatersNodeDownload({ walletConnected }: { walletConnected: boolean }) {
  const [showTooltip, setShowTooltip] = useState(false);
  const handleClick = () => {
    if (!walletConnected) return;
    window.open("/download/waters-node-v0.2.0-linux-x64.tar.gz", "_blank");
  };
  return <>
    <div style={{position:"fixed",bottom:"70px",left:"20px",zIndex:9999}}>
      <a href="https://github.com/waters-ai/waters-node" target="_blank"
        style={{color:"#00d4ff",fontSize:"12px",fontFamily:"system-ui,monospace",opacity:0.7,textDecoration:"none"}}>
        waters-node v0.2.0 - analog TUI →
      </a>
    </div>
    <div style={{position:"fixed",bottom:"20px",left:"20px",zIndex:9999}}
      onMouseEnter={() => setShowTooltip(true)}
      onMouseLeave={() => setShowTooltip(false)}>
      <button onClick={handleClick} style={{
        background: walletConnected ? "linear-gradient(135deg,#00d4ff,#0088cc)" : "rgba(255,255,255,0.08)",
        color: walletConnected ? "#000" : "rgba(255,255,255,0.3)",
        border: walletConnected ? "1px solid #00d4ff" : "1px solid rgba(255,255,255,0.15)",
        borderRadius:"12px",padding:"10px 18px",cursor: walletConnected ? "pointer" : "not-allowed",
        fontFamily:"system-ui,monospace",fontSize:"13px",fontWeight:600,
        transition:"all 0.3s ease",opacity: walletConnected ? 1 : 0.5,
        display:"flex",alignItems:"center",gap:"8px"}}>
        <span style={{fontSize:"18px"}}>🌊</span>
        <span>waters-node</span>
        <span style={{fontSize:"10px",opacity:0.7}}>v0.2.0</span>
      </button>
      {showTooltip && !walletConnected && (
        <div style={{position:"absolute",bottom:"100%",left:0,marginBottom:"8px",
          background:"rgba(255,255,255,0.1)",backdropFilter:"blur(10px)",
          border:"1px solid rgba(255,255,255,0.15)",borderRadius:"8px",padding:"8px 14px",
          fontSize:"12px",color:"#ff6b6b",whiteSpace:"nowrap",fontFamily:"system-ui,monospace"}}>
          ⚡ requires wallet connection
        </div>
      )}
    </div>
  </>;
}'''

content = content[:idx] + new_func + content[end_idx:]
open("/home/constructor/projects/kapelka/src/pages/Home.tsx", "w").write(content)
print("Done")
