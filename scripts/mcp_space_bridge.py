#!/usr/bin/env python3
"""
mcp_space_bridge.py — MCP-сервер космической миссии WATERS

Протокол: MCP stdio (JSON-RPC 2.0 через stdin/stdout)
Доступ к:
  - Neo4j (237) — граф онтологии миссии
  - SPICE kernels (237) — эфемериды NASA/CNSA
  - SQLite rules (238) — правила SLA/стоимости
  - Redis (238) — кэш космических расчётов
  - Kafka (238) — медленное наполнение данными

Запуск: OpenCode запускает как subprocess через mcpServers в opencode.json
"""

import asyncio
import json
import logging
import os
import sqlite3
import subprocess
import sys
import uuid
from datetime import datetime, timezone

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [space-bridge] %(message)s",
)
logger = logging.getLogger("space-bridge")

SSH_HOST = os.environ.get("SPACE_SSH_HOST", "171.22.180.237")
SSH_USER = os.environ.get("SPACE_SSH_USER", "ubuntu")
RULES_HOST = os.environ.get("RULES_HOST", "171.22.180.238")
RULES_USER = os.environ.get("RULES_USER", "ubuntu")
NEO4J_URI = os.environ.get("NEO4J_URI", "bolt://localhost:17687")
NEO4J_AUTH = os.environ.get("NEO4J_AUTH", ("neo4j", "waters_mission"))
REDIS_HOST = os.environ.get("REDIS_HOST", "localhost")
KAFKA_BOOTSTRAP = os.environ.get("KAFKA_BOOTSTRAP", "localhost:9092")
RULES_DB_PATH = os.environ.get("RULES_DB_PATH", "/data/waters_rules.db")

# Lazy clients
_neo4j_driver = None
_redis = None
_kafka_producer = None


def ssh_run(cmd, host=None, user=None):
    host = host or SSH_HOST
    user = user or SSH_USER
    result = subprocess.run(
        ["ssh", f"{user}@{host}", cmd],
        capture_output=True, text=True, timeout=30
    )
    if result.returncode != 0:
        raise RuntimeError(f"SSH error: {result.stderr.strip()}")
    return result.stdout.strip()


def ssh_python(code):
    return ssh_run(f"python3 -c {sh_quote(code)}")


def sh_quote(s):
    return "'" + s.replace("'", "'\\''") + "'"


def get_rules_db():
    try:
        local_path = "/tmp/waters_rules.db"
        subprocess.run(
            ["scp", f"{RULES_USER}@{RULES_HOST}:{RULES_DB_PATH}", local_path],
            capture_output=True, timeout=10
        )
        return sqlite3.connect(local_path)
    except Exception as e:
        logger.warning(f"Rules DB unavailable: {e}")
        return None


# ── Tool handlers ──

def tool_ephemeris(args):
    target = args.get("target", "MOON")
    observer = args.get("observer", "EARTH")
    time_str = args.get("time", "NOW")

    payload = json.dumps({"target": target, "observer": observer, "time": time_str})
    cmd = f"echo '{payload}' | python3 /home/ubuntu/spice_query.py"
    try:
        result = ssh_run(cmd)
        return json.loads(result)
    except Exception as e:
        return {"error": str(e)}


def tool_mission_query(args):
    query = args.get("query", "MATCH (n:CelestialBody) RETURN n.id, n.name LIMIT 10")
    try:
        from neo4j import GraphDatabase
        driver = GraphDatabase.driver(NEO4J_URI, auth=NEO4J_AUTH)
        with driver.session() as session:
            result = session.run(query)
            records = [dict(r) for r in result]
        driver.close()
        return {"records": records, "count": len(records)}
    except Exception as e:
        return {"error": str(e)}


def tool_rules_query(args):
    table = args.get("table", "sla_rules")
    db = get_rules_db()
    if not db:
        return {"error": "Rules DB not available"}

    try:
        c = db.cursor()
        c.execute(f"SELECT name FROM sqlite_master WHERE type='table' AND name=?", (table,))
        if not c.fetchone():
            return {"error": f"Table '{table}' not found"}
        c.execute(f"SELECT * FROM {table}")
        rows = c.fetchall()
        cols = [d[0] for d in c.description]
        db.close()
        return {"table": table, "columns": cols, "rows": rows, "count": len(rows)}
    except Exception as e:
        db.close()
        return {"error": str(e)}


def tool_load_ontology(args):
    """Load lunar mission ontology into Neo4j"""
    try:
        from neo4j import GraphDatabase
        driver = GraphDatabase.driver(NEO4J_URI, auth=NEO4J_AUTH)

        with open("ontology/lunar_mission.jsonld") as f:
            ontology = json.load(f)

        with driver.session() as session:
            # Create constraints
            session.run("CREATE CONSTRAINT IF NOT EXISTS FOR (c:CelestialBody) REQUIRE c.id IS UNIQUE")
            session.run("CREATE CONSTRAINT IF NOT EXISTS FOR (a:LunarAsset) REQUIRE a.id IS UNIQUE")
            session.run("CREATE CONSTRAINT IF NOT EXISTS FOR (m:LunarMission) REQUIRE m.id IS UNIQUE")

            ontology_classes = ontology.get("ontology", {}).get("classes", [])
            for cls in ontology_classes:
                cls_id = cls.get("@id", "")
                cls_type = cls.get("@type", "")
                cls_name = cls_id.split(":")[-1] if ":" in cls_id else cls_id
                cls_comment = cls.get("comment", "")

                # Map to Neo4j labels
                if "CelestialBody" in cls_name:
                    label = "CelestialBody"
                elif "LunarAsset" in cls_name:
                    label = "LunarAsset"
                elif "AgentRole" in cls_name:
                    label = "AgentRole"
                elif "LunarMission" in cls_name:
                    label = "LunarMission"
                else:
                    continue

                session.run(
                    f"MERGE (n:{label} {{id: $id}}) SET n.name = $name, n.description = $comment",
                    id=cls_name, name=cls_name, comment=cls_comment
                )

            # Load properties/relationships
            props = ontology.get("ontology", {}).get("properties", [])
            for prop in props:
                domain = prop.get("domain", {}).get("@id", "")
                range_val = prop.get("range", {}).get("@id", "")
                prop_name = prop.get("@id", "").split(":")[-1] if ":" in prop.get("@id", "") else prop.get("@id", "")

                if domain and range_val:
                    d_label = domain.split(":")[-1]
                    r_label = range_val.split(":")[-1]
                    session.run(
                        f"MATCH (a {{id: $domain}}), (b {{id: $range}})"
                        f" MERGE (a)-[r:{prop_name.upper()}]->(b) SET r.type = $prop",
                        domain=d_label, range=r_label, prop=prop_name
                    )

        driver.close()
        return {"status": "loaded", "classes": len(ontology_classes)}
    except Exception as e:
        return {"error": str(e)}


def tool_kafka_feed(args):
    """Send data to Kafka for slow distribution to other agents"""
    topic = args.get("topic", "space.ephemeris.v1")
    data = args.get("data", {})
    ttl_hours = args.get("ttl_hours", 24)

    try:
        from kafka import KafkaProducer
        producer = KafkaProducer(
            bootstrap_servers=KAFKA_BOOTSTRAP,
            value_serializer=lambda v: json.dumps(v, ensure_ascii=False).encode(),
            acks="all",
        )
        msg = {
            "msg_id": str(uuid.uuid4()),
            "ts": datetime.now(timezone.utc).isoformat(),
            "ttl_hours": ttl_hours,
            **data,
        }
        producer.send(topic, msg)
        producer.flush()
        return {"status": "sent", "topic": topic, "msg_id": msg["msg_id"]}
    except Exception as e:
        return {"error": str(e)}


TOOLS = {
    "space_ephemeris": {
        "fn": tool_ephemeris,
        "description": "Положение небесного тела на дату (SPICE на 237)",
        "inputSchema": {
            "type": "object",
            "properties": {
                "target": {"type": "string", "default": "MOON", "description": "Целевое тело (MOON, SUN, MARS, etc)"},
                "observer": {"type": "string", "default": "EARTH", "description": "Наблюдатель (EARTH, MOON, SUN)"},
                "time": {"type": "string", "default": "NOW", "description": "Время UTC (ISO8601 или NOW)"},
            },
        },
    },
    "space_mission_query": {
        "fn": tool_mission_query,
        "description": "Cypher-запрос к графу миссии в Neo4j на 237",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "default": "MATCH (n) RETURN n LIMIT 10", "description": "Cypher query"},
            },
            "required": ["query"],
        },
    },
    "space_rules_query": {
        "fn": tool_rules_query,
        "description": "Запрос к SQLite с правилами (schemas, sla_rules, cost_limits)",
        "inputSchema": {
            "type": "object",
            "properties": {
                "table": {"type": "string", "default": "sla_rules", "description": "Таблица: schemas, sla_rules, cost_limits"},
            },
        },
    },
    "space_load_ontology": {
        "fn": tool_load_ontology,
        "description": "Загрузить онтологию лунной миссии в Neo4j",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    },
    "space_kafka_feed": {
        "fn": tool_kafka_feed,
        "description": "Отправить данные в Kafka для медленного наполнения всех агентов",
        "inputSchema": {
            "type": "object",
            "properties": {
                "topic": {"type": "string", "default": "space.ephemeris.v1"},
                "data": {"type": "object", "description": "Данные для отправки"},
                "ttl_hours": {"type": "number", "default": 24},
            },
            "required": ["data"],
        },
    },
}


def send_message(msg):
    line = json.dumps(msg, ensure_ascii=False)
    sys.stdout.write(line + "\n")
    sys.stdout.flush()


def handle_request(request):
    method = request.get("method", "")
    params = request.get("params", {})
    req_id = request.get("id")

    if method == "initialize":
        send_message({
            "jsonrpc": "2.0", "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        name: {
                            "name": name,
                            "description": info["description"],
                            "inputSchema": info["inputSchema"],
                        }
                        for name, info in TOOLS.items()
                    }
                },
                "serverInfo": {"name": "waters-space-bridge", "version": "1.0.0"},
            },
        })
        return

    if method == "initialized":
        logger.info("MCP client initialized")
        return

    if method == "tools/list":
        send_message({
            "jsonrpc": "2.0", "id": req_id,
            "result": {
                "tools": [
                    {"name": name, "description": info["description"],
                     "inputSchema": info["inputSchema"]}
                    for name, info in TOOLS.items()
                ]
            },
        })
        return

    if method == "tools/call":
        tool_name = params.get("name", "")
        arguments = params.get("arguments", {})

        tool = TOOLS.get(tool_name)
        if not tool:
            send_message({
                "jsonrpc": "2.0", "id": req_id,
                "error": {"code": -32601, "message": f"Tool not found: {tool_name}"},
            })
            return

        try:
            result = tool["fn"](arguments)
            send_message({
                "jsonrpc": "2.0", "id": req_id,
                "result": {"content": [{"type": "text", "text": json.dumps(result, ensure_ascii=False)}]},
            })
        except Exception as e:
            logger.exception(f"Tool error: {tool_name}")
            send_message({
                "jsonrpc": "2.0", "id": req_id,
                "error": {"code": -32603, "message": str(e)},
            })
        return

    logger.warning(f"Unknown method: {method}")
    send_message({
        "jsonrpc": "2.0", "id": req_id,
        "error": {"code": -32601, "message": f"Method not found: {method}"},
    })


def main():
    logger.info("=== WATERS Space Bridge MCP Server ===")
    logger.info(f"SSH: {SSH_USER}@{SSH_HOST}")
    logger.info(f"Neo4j: {NEO4J_URI}")
    logger.info(f"Kafka: {KAFKA_BOOTSTRAP}")
    logger.info(f"Tools loaded: {len(TOOLS)}")
    logger.info("Ready on stdin/stdout (MCP stdio protocol)")

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            handle_request(request)
        except json.JSONDecodeError as e:
            logger.warning(f"Invalid JSON: {e}")
        except Exception as e:
            logger.exception(f"Fatal error: {e}")


if __name__ == "__main__":
    main()
