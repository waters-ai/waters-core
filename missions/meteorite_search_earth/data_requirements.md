# 🌐 Требования к данным / Data Requirements / 数据需求

**Дата:** 08.05.2026
**Статус:** Доктрина (Фаза 0)
**Автор:** Архитектор v1.0 / Конструктор Сети v1.0

---

## 1. NASA SPICE / NASA SPICE / NASA SPICE

### 1.1 Необходимые ядра / Required Kernels / 必需核心

| Ядро / Kernel / 核心 | Тип / Type / 类型 | Назначение / Purpose / 用途 | Размер / Size / 大小 |
|:---|:---|:---|:---|
| `de440.bsp` | Эфемериды планет | Положение Земли, Луны, Солнца | ~120 MB |
| `de440s.bsp` | Эфемериды (сокращённые) | Быстрый расчёт для реального времени | ~25 MB |
| `lunar.bsp` | Эфемериды Луны | Положение и ориентация Луны | ~10 MB |
| `earth_000101_250104.bpc` | Ориентация Земли | Вращение Земли, полюс, UT1 | ~5 MB |
| `moon_pa_de440_200730.bpc` | Ориентация Луны | Либрация, полюс Луны | ~3 MB |
| `earth_latest_high_prec.bpc` | Ориентация Земли (high-prec) | Высокоточная ориентация для RTK | ~15 MB |
| `pck00010.tpc` | Физические константы | Радиусы, гравитация, параметры | ~2 MB |
| `gm_de440.tpc` | Гравитационные параметры | GM для расчётов орбит | ~1 MB |

### 1.2 Инструменты / Tools / 工具

| Инструмент / Tool / 工具 | Назначение / Purpose / 用途 | Статус / Status / 状态 |
|:---|:---|:---|
| **NAIF Toolkit (CSPICE)** | Базовые SPICE-расчёты | ✅ Развёрнут на 237 |
| **Orekit** | Java-библиотека орбитальной механики | ✅ Развёрнут на 237 |
| **GMAT** | Миссионное планирование, траектории | 🔄 Установка |
| **spiceypy** | Python-обёртка для CSPICE | ✅ Через MCP bridge |

### 1.3 Источники / Sources / 来源

| Источник / Source / 来源 | URL | Частота обновления / Update / 更新频率 |
|:---|:---|:---|
| NAIF (NASA) | `https://naif.jpl.nasa.gov/pub/naif/generic_kernels/` | По мере выхода |
| Planetary Data System | `https://pds.nasa.gov/` | Ежеквартально |
| ESA SPICE Service | `https://spice.esac.esa.int/` | Ежемесячно |

---

## 2. CNSA (Chang'e Program) / CNSA (嫦娥计划) / CNSA (嫦娥计划)

### 2.1 Доступные данные / Available Data / 可用数据

| Миссия / Mission / 任务 | Данные / Data / 数据 | Статус доступа / Access / 访问状态 |
|:---|:---|:---|
| Chang'e-3 | Посадочный радар, панорамы | 🔄 Ожидание доступа |
| Chang'e-4 (Far side) | Yutu-2 ровер, спектры VNIS | 🔄 Ожидание доступа |
| Chang'e-5 | Образцы грунта (1.7 kg) | 🔄 Данные по запросу |
| Chang'e-6 | Образцы с обратной стороны | 🔄 Ожидание (2026) |
| Chang'e-7 | Орбитальный радар, спектрометр | 🔄 Планируется (2026) |
| Chang'e-8 | 3D-печать, ресурсы | 🔄 Планируется (2028+) |

### 2.2 Интеграция SpaceMind / SpaceMind Integration / SpaceMind 集成

| Компонент / Component / 组件 | Описание / Description / 描述 | Статус / Status / 状态 |
|:---|:---|:---|
| **SPICE-формат CNSA** | Конвертация CNSA-данных в SPICE-совместимый формат | 🔄 Разработка |
| **SpaceMind API** | Доступ к данным Chang'e через API | 🔄 Согласование |
| **Китайские геологические карты** | Цифровые модели рельефа, спектральные данные | 🔄 Ожидание |

---

## 3. Геологические карты / Geological Maps / 地质图

### 3.1 USGS Astrogeology / USGS Astrogeology / USGS 天文地质

| Набор / Dataset / 数据集 | Описание / Description / 描述 | Разрешение / Resolution / 分辨率 | Размер / Size / 大小 |
|:---|:---|:---|:---|
| **Lunar Map Catalog** | Геологические карты Луны (1:5M — 1:250K) | 100-500 m/px | ~50 GB |
| **Moon LRO LOLA** | Цифровая модель рельефа (DEM) | 100 m/px | ~30 GB |
| **Moon LRO NAC** | Высокодетальные снимки полюсов | 0.5 m/px | ~500 GB |
| **Mars HRSC** | DEM Марса (для сравнения) | 50 m/px | ~20 GB |
| **Earth SRTM** | DEM Земли (полигон Атакама) | 30 m/px | ~2 GB |

### 3.2 Planetary Data System (PDS) / PDS / 行星数据系统

| Узел / Node / 节点 | Данные / Data / 数据 | Протокол / Protocol / 协议 |
|:---|:---|:---|
| **Geosciences Node** | Геологические данные, спектры | HTTPS / FTP |
| **Atmospheres Node** | Атмосферные данные (для Земли) | HTTPS |
| **Imaging Node** | Снимки, карты | HTTPS / PDS API |
| **Small Bodies Node** | Метеориты, астероиды | HTTPS |

---

## 4. Спектральные библиотеки / Spectral Libraries / 光谱库

### 4.1 RELAB / RELAB / RELAB (布朗大学)

| Параметр / Parameter / 参数 | Значение / Value / 值 |
|:---|:---|
| **URL** | `https://sites.brown.edu/relab/` |
| **Спектры** | > 50,000 образцов (минералы, реголит, метеориты) |
| **Диапазон** | UV (0.3 µm) — MIR (50 µm) |
| **Формат** | ASCII, CSV |
| **Доступ** | Открытый (API по запросу) |
| **Интеграция** | `memory_vector_search("relab:mineral_name", collection="spectra")` |

### 4.2 ECOSTRESS / ECOSTRESS / ECOSTRESS (NASA)

| Параметр / Parameter / 参数 | Значение / Value / 值 |
|:---|:---|
| **URL** | `https://ecostress.jpl.nasa.gov/` |
| **Спектры** | > 3,000 образцов (минералы, растительность, вода) |
| **Диапазон** | TIR (8-12 µm) |
| **Доступ** | Открытый |
| **Формат** | HDF5, JSON |

### 4.3 HYPEX / HYPEX / HYPEX (Гиперспектральные библиотеки)

| Параметр / Parameter / 参数 | Значение / Value / 值 |
|:---|:---|
| **Назначение** | Гиперспектральные библиотеки для анализа дронов |
| **Диапазон** | VNIR (0.4-1.0 µm), SWIR (1.0-2.5 µm) |
| **Формат** | ENVI, GeoTIFF |
| **Интеграция** | Загрузка через MCP bridge в Qdrant/ChromaDB |

---

## 5. Метеоритные базы / Meteorite Databases / 陨石数据库

| База / Database / 数据库 | URL | Записей / Records / 记录 |
|:---|:---|:---|
| **Meteoritical Bulletin** | `https://www.lpi.usra.edu/meteor/` | > 70,000 |
| **NASA Meteorite Catalog** | `https://data.nasa.gov/Space-Science/Meteorite-Landings/` | > 50,000 |
| **Atacama Meteorite Field** | Специализированная база по Атакаме | > 5,000 |

---

## 6. Данные для земной миссии / Earth Mission Data / 地球任务数据

| Тип / Type / 类型 | Источник / Source / 来源 | Размер / Size / 大小 | Формат / Format / 格式 |
|:---|:---|:---|:---|
| DEM полигона (Атакама) | SRTM 30m / ALOS 12m | ~500 MB | GeoTIFF |
| Спутниковые снимки | Sentinel-2 (10m), Planet (3m) | ~2 GB | GeoTIFF, JPEG2000 |
| Геологические карты Чили | SERNAGEOMIN | ~100 MB | Shapefile, GeoJSON |
| Метеоритные находки Атакамы | MILAGRO project | ~5 MB | CSV, GeoJSON |
| Гидрогеология | Solares, aquifer maps | ~50 MB | Shapefile |

---

## 7. Инфраструктура хранения / Storage Infrastructure / 存储基础设施

| База данных / Database / 数据库 | Объём данных / Capacity / 容量 | Тип данных / Data Type / 数据类型 |
|:---|:---|:---|:---|
| **TimescaleDB** | ~10 GB (телеметрия 30 дней) | Временные ряды (50 Hz × 4 дрона) |
| **Neo4j** | ~5 GB (граф состояний) | Связи агент-локация-событие |
| **ChromaDB** | ~2 GB (векторы спектров) | Эмбеддинги спектральных библиотек |
| **LightRAG** | ~1 GB (граф знаний) | Онтология миссии, статьи |
| **Redis** | ~500 MB (кэш, сессии) | Кэш SPICE-расчётов, сессии агентов |

---

## 8. Процесс загрузки данных / Data Ingestion Process / 数据加载流程

```
Внешний источник ──▶ Скрипт загрузки ──▶ Kafka ──▶ База данных
       │                                        │
       └──▶ (ручная загрузка через MCP)         └──▶ (автоматическая конверсия)
```

1. NASA SPICE: `scripts/load_spice_kernels.sh` → SPICE toolkit → `orbits.calculated.v1` (Kafka)
2. USGS карты: `scripts/load_usgs_maps.sh` → Neo4j → LightRAG
3. RELAB спектры: `scripts/load_relab_spectra.py` → ChromaDB (коллекция `spectra`)
4. Геоданные Чили: `scripts/load_chile_geology.sh` → TimescaleDB → Neo4j

---

*Данные — топливо для миссии. Без SPICE навигатор слеп. Без спектров геолог глух.*
*Data is the fuel for the mission. Without SPICE the navigator is blind. Without spectra the geologist is deaf.*
*数据是任务的燃料。没有 SPICE，导航员是盲的。没有光谱，地质学家是聋的。*
