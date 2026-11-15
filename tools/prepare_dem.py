#!/usr/bin/env python3
"""STAR: подготовка DEM (Земля / Луна) для star_backend. Версия на rasterio
(GDAL внутри pip-пакета, отдельная установка GDAL не нужна).

Установка:   pip install rasterio numpy

Что делает (ТЗ, раздел 5, «Подготовка»):
  1. Читает один или несколько входных растров DEM (мозаика, если их несколько).
  2. Перепроецирует в метрическую систему:
       Земля -> локальная UTM (зона по центру bbox/растра),
       Луна  -> полярная стереография (радиус 1737400 м), южный полюс.
  3. Приводит к единой сетке: --res 90 (обзор), --res-fine 30 (топ-кандидаты).
  4. Пишет GeoTIFF, который читает load_geotiff(): один канал, Float32,
     без сжатия, квадратные пиксели, NoData = -9999.
  5. Пишет рядом <имя>.meta.json: источники, CRS, разрешение, версия GDAL,
     параметры (история преобразований).

Примеры:
  python tools/prepare_dem.py --body earth --region Atacama ^
      --input srtm_atacama.tif --bbox -69.6 -24.8 -69.0 -24.2 --res 90 --res-fine 30
  python tools/prepare_dem.py --body moon --region Haworth ^
      --input LDEM_80S_20M.tif --res 90
"""
import argparse
import datetime
import json
import math
import os
import sys

try:
    import numpy as np
    import rasterio
    from rasterio.crs import CRS
    from rasterio.transform import from_origin
    from rasterio.warp import Resampling, reproject, transform_bounds
except ImportError:
    sys.exit("Нужны пакеты: pip install rasterio numpy")

NODATA = -9999.0
MOON_RADIUS_M = 1737400
MOON_SOUTH_POLAR = (
    f"+proj=stere +lat_0=-90 +lat_ts=-90 +lon_0=0 +x_0=0 +y_0=0 "
    f"+R={MOON_RADIUS_M} +units=m +no_defs"
)
CELL_LIMIT = 100_000_000


def utm_epsg(lon, lat):
    zone = max(1, min(60, int((lon + 180) // 6) + 1))
    return (32600 if lat >= 0 else 32700) + zone


def safe(name):
    return "".join(c if c.isalnum() or c in "-_" else "_" for c in name)


def target_crs(args, sources):
    if args.t_srs:
        s = args.t_srs
        return CRS.from_string(s), s
    if args.body == "moon":
        return CRS.from_proj4(MOON_SOUTH_POLAR), MOON_SOUTH_POLAR
    if args.bbox:
        lon = (args.bbox[0] + args.bbox[2]) / 2
        lat = (args.bbox[1] + args.bbox[3]) / 2
    else:
        b = [transform_bounds(s.crs, "EPSG:4326", *s.bounds) for s in sources]
        lon = (min(x[0] for x in b) + max(x[2] for x in b)) / 2
        lat = (min(x[1] for x in b) + max(x[3] for x in b)) / 2
    if abs(lat) > 84:
        sys.exit("Широта > 84°: UTM неприменима, задайте --t-srs (полярная стереография).")
    code = utm_epsg(lon, lat)
    return CRS.from_epsg(code), f"EPSG:{code}"


def dest_bounds(args, sources, dst_crs):
    """Границы выходной сетки в целевой СК, выровненные по шагу."""
    if args.bbox and args.body == "earth":
        w, s, e, n = args.bbox
        return transform_bounds("EPSG:4326", dst_crs, w, s, e, n)
    bs = [transform_bounds(s.crs, dst_crs, *s.bounds) for s in sources]
    return (min(b[0] for b in bs), min(b[1] for b in bs),
            max(b[2] for b in bs), max(b[3] for b in bs))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--body", choices=["earth", "moon"], required=True)
    ap.add_argument("--region", required=True, help="Имя региона/района (идёт в имя файла)")
    ap.add_argument("--input", nargs="+", required=True, help="Один или несколько растров DEM")
    ap.add_argument("--bbox", nargs=4, type=float, metavar=("W", "S", "E", "N"),
                    help="Земля: обрезка в градусах lon/lat")
    ap.add_argument("--res", type=float, default=90.0, help="Обзорное разрешение, м (по умолчанию 90)")
    ap.add_argument("--res-fine", type=float, default=None, help="Доп. разрешение для топ-кандидатов, м (например 30)")
    ap.add_argument("--out", default=None, help="Папка вывода (по умолчанию data/<body>)")
    ap.add_argument("--dem-name", default=None, help="Источник для meta.json (SRTM, ASTER GDEM, COP30, LOLA)")
    ap.add_argument("--t-srs", default=None, help="Свой целевой CRS (EPSG:xxxx или proj4)")
    ap.add_argument("--resample", default="average",
                    help="Метод ресэмплинга: average (укрупнение пикселя), bilinear, nearest, cubic...")
    args = ap.parse_args()

    for p in args.input:
        if not os.path.exists(p):
            sys.exit(f"Файл не найден: {p}")
    try:
        resampling = Resampling[args.resample]
    except KeyError:
        sys.exit(f"Неизвестный метод ресэмплинга: {args.resample}")

    out_dir = args.out or os.path.join("data", args.body)
    os.makedirs(out_dir, exist_ok=True)
    region = safe(args.region)

    sources = [rasterio.open(p) for p in args.input]
    for p, s in zip(args.input, sources):
        if s.crs is None:
            sys.exit(f"У растра {p} нет системы координат.")
    dst_crs, t_srs_label = target_crs(args, sources)
    print(f"Целевая СК: {t_srs_label}")
    bounds = dest_bounds(args, sources, dst_crs)

    resolutions = [args.res] + ([args.res_fine] if args.res_fine else [])
    outputs = []
    for res in resolutions:
        res_label = f"{res:g}m"
        left = math.floor(bounds[0] / res) * res
        top = math.ceil(bounds[3] / res) * res
        width = int(math.ceil((bounds[2] - left) / res))
        height = int(math.ceil((top - bounds[1]) / res))
        cells = width * height
        print(f"\n[{res_label}] сетка {width}x{height} = {cells:,} ячеек")
        if cells > CELL_LIMIT:
            sys.exit(f"Больше {CELL_LIMIT:,} ячеек (лимит загрузчика). "
                     "Уменьшите bbox или увеличьте --res.")
        transform = from_origin(left, top, res, res)
        dst = np.full((height, width), NODATA, dtype=np.float32)
        for i, src in enumerate(sources):
            reproject(
                source=rasterio.band(src, 1),
                destination=dst,
                src_nodata=src.nodata,
                dst_nodata=NODATA,
                dst_transform=transform,
                dst_crs=dst_crs,
                resampling=resampling,
                init_dest_nodata=(i == 0),
            )
        valid = int((dst != NODATA).sum())
        if valid == 0:
            sys.exit("После перепроецирования нет валидных ячеек: проверьте bbox и вход.")

        out_tif = os.path.join(out_dir, f"{region}_{res_label}.tif")
        profile = dict(driver="GTiff", dtype="float32", count=1, width=width, height=height,
                       crs=dst_crs, transform=transform, nodata=NODATA,
                       compress=None, tiled=False, BIGTIFF="IF_SAFER")
        with rasterio.open(out_tif, "w", **profile) as dstf:
            dstf.write(dst, 1)
        print(f"  Результат: {out_tif}  (валидных {valid:,} из {cells:,}, шаг {res:g} м)")

        meta_out = {
            "schema_version": "star-prep-1",
            "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
            "body": args.body,
            "region": args.region,
            "dem": args.dem_name or "unknown",
            "inputs": [os.path.relpath(p).replace(os.sep, "/") for p in args.input],
            "output": os.path.relpath(out_tif).replace(os.sep, "/"),
            "target_srs": t_srs_label,
            "resolution_m": res,
            "resample": args.resample,
            "bbox_lonlat": args.bbox,
            "nodata": NODATA,
            "size_px": [width, height],
            "valid_cells": valid,
            "rasterio": rasterio.__version__,
            "gdal": rasterio.__gdal_version__,
        }
        with open(out_tif.replace(".tif", ".meta.json"), "w", encoding="utf-8") as f:
            json.dump(meta_out, f, ensure_ascii=False, indent=2)
        outputs.append(out_tif)

    print("\nГотово:")
    for o in outputs:
        print("  ", o)
    print("Дальше: загрузите файл в приложении (Terrain Analysis -> Load GeoTIFF).")


if __name__ == "__main__":
    main()
