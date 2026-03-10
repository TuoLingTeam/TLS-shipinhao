# -*- coding: utf-8 -*-
"""TLS-shipinhao 配置文件管理。"""

import os
import re
import sys

from .constants import (
    CONFIG_DIR_NAME,
    COOKIE_FILE_NAME,
    COOKIE_FILE_STEM,
    MAGIC_FILE_NAME,
    MAGIC_FILE_STEM,
    USER_CONFIG_POINTER,
)

# ---------------------------------------------------------------------------
# 配置目录缓存
# ---------------------------------------------------------------------------
_CONFIG_DIR_CACHE = None


class ConfigNotFoundError(FileNotFoundError):
    """配置文件缺失时抛出更明确的错误。"""

    def __init__(self, searched_dirs):
        self.searched_dirs = searched_dirs
        super().__init__("未找到配置文件。")


# ---------------------------------------------------------------------------
# 目录解析
# ---------------------------------------------------------------------------


def get_app_dir():
    """获取 .app 同级目录或源码项目根目录。"""
    if getattr(sys, "frozen", False):
        exe_dir = os.path.abspath(os.path.dirname(sys.executable))
        if sys.platform == "darwin":
            bundle_root = os.path.abspath(os.path.join(exe_dir, "..", ".."))
            if bundle_root.endswith(".app"):
                return os.path.dirname(bundle_root)
        return exe_dir
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def get_home_config_dir():
    """获取用户主目录下的固定配置目录。"""
    return os.path.join(os.path.expanduser("~"), CONFIG_DIR_NAME)


def get_user_config_pointer_path():
    """记录用户指定配置目录的指针文件。"""
    return os.path.join(get_home_config_dir(), USER_CONFIG_POINTER)


def get_saved_user_config_dir():
    """读取用户上次选择的配置目录。"""
    pointer_path = get_user_config_pointer_path()
    if not os.path.exists(pointer_path):
        return None
    with open(pointer_path, "r", encoding="utf-8") as file:
        selected_dir = file.read().strip()
    if selected_dir and os.path.isdir(selected_dir):
        return selected_dir
    return None


def save_user_config_dir(config_dir):
    """保存用户指定的配置目录。"""
    global _CONFIG_DIR_CACHE
    config_dir = os.path.abspath(config_dir)
    os.makedirs(get_home_config_dir(), exist_ok=True)
    with open(get_user_config_pointer_path(), "w", encoding="utf-8") as file:
        file.write(config_dir)
    _CONFIG_DIR_CACHE = config_dir


def get_config_search_dirs():
    """仅返回用户手动选择过的配置目录。"""
    selected_dir = _CONFIG_DIR_CACHE or get_saved_user_config_dir()
    if not selected_dir:
        return []
    return [os.path.abspath(selected_dir)]


def get_config_dir_cache():
    """获取当前配置目录缓存值。"""
    return _CONFIG_DIR_CACHE


# ---------------------------------------------------------------------------
# 配置文件名识别
# ---------------------------------------------------------------------------


def _strip_txt_suffixes(filename):
    """去掉文件名末尾连续 .txt 后缀（兼容 Windows 双后缀场景）。"""
    normalized = (filename or "").strip().lower()
    while normalized.endswith(".txt"):
        normalized = normalized[:-4]
    return normalized


def _classify_config_file_name(filename):
    """识别配置文件类型：cookie 或 biz_magic。"""
    stem = _strip_txt_suffixes(filename)
    if stem == COOKIE_FILE_STEM:
        return "cookie"
    if stem == MAGIC_FILE_STEM:
        return "magic"
    return None


def _config_file_priority(filename, file_type):
    """配置文件命名优先级：标准名 > 无后缀名 > 其它可识别变体。"""
    normalized = (filename or "").strip().lower()
    if file_type == "cookie":
        if normalized == COOKIE_FILE_NAME:
            return 30
        if normalized == COOKIE_FILE_STEM:
            return 20
        return 10
    if normalized == MAGIC_FILE_NAME:
        return 30
    if normalized == MAGIC_FILE_STEM:
        return 20
    return 10


# ---------------------------------------------------------------------------
# 配置文件解析
# ---------------------------------------------------------------------------


def resolve_config_files_in_dir(config_dir):
    """在目录中解析配置文件路径（cookie 必需，biz_magic 可选）。"""
    if not config_dir or not os.path.isdir(config_dir):
        return None

    resolved = {}
    try:
        filenames = os.listdir(config_dir)
    except OSError:
        return None

    for filename in filenames:
        full_path = os.path.join(config_dir, filename)
        if not os.path.isfile(full_path):
            continue

        file_type = _classify_config_file_name(filename)
        if not file_type:
            continue

        current_path = resolved.get(file_type)
        if current_path is None:
            resolved[file_type] = full_path
            continue

        current_name = os.path.basename(current_path)
        if _config_file_priority(filename, file_type) > _config_file_priority(current_name, file_type):
            resolved[file_type] = full_path

    if "cookie" not in resolved:
        return None
    return {"cookie": resolved["cookie"], "magic": resolved.get("magic")}


def parse_cookie_content(content):
    """将 cookie 原文解析为字典。"""
    pairs = content.split(";")
    data = {}
    for pair in pairs:
        if "=" in pair:
            key, value = pair.strip().split("=", 1)
            data[key.strip()] = value.strip()
    return data


def read_cookie_data(cookie_path):
    """读取 cookie 文件并解析成字典。"""
    with open(cookie_path, "r", encoding="utf-8") as file:
        content = file.read().strip()
    return parse_cookie_content(content)


def read_magic_file(magic_path):
    """读取独立 biz_magic 文件。"""
    with open(magic_path, "r", encoding="utf-8") as file:
        return file.read().strip()


def extract_biz_magic_from_cookie(cookie_data):
    """从 cookie 字典中提取 biz_magic（大小写不敏感）。"""
    direct = cookie_data.get("biz_magic")
    if direct:
        return str(direct).strip()

    for key, value in cookie_data.items():
        if str(key).strip().lower() == "biz_magic" and str(value).strip():
            return str(value).strip()
    return ""


def is_config_dir_ready(config_dir):
    """判断目录是否可用：有 cookie.txt 文件且包含 biz_magic 值。"""
    file_paths = resolve_config_files_in_dir(config_dir)
    if not file_paths:
        return False

    try:
        cookie_data = read_cookie_data(file_paths["cookie"])
        return bool(extract_biz_magic_from_cookie(cookie_data))
    except Exception:  # noqa: BLE001
        return False


def resolve_config_dir():
    """解析实际可用的配置目录（仅用户手选目录）。"""
    global _CONFIG_DIR_CACHE
    if _CONFIG_DIR_CACHE and is_config_dir_ready(_CONFIG_DIR_CACHE):
        return _CONFIG_DIR_CACHE

    search_dirs = get_config_search_dirs()
    if not search_dirs:
        raise ConfigNotFoundError(["未选择配置目录，请先点击「选择配置目录」。"])

    for config_dir in search_dirs:
        if is_config_dir_ready(config_dir):
            _CONFIG_DIR_CACHE = config_dir
            return config_dir
    raise ConfigNotFoundError(
        [
            f"当前选择目录：{search_dirs[0]}",
            "该目录下未找到可用配置（需 cookie.txt 文件）。",
        ]
    )


def get_cookie():
    """读取 Cookie 配置（兼容 cookie/cookie.txt/cookie.txt.txt）。"""
    config_dir = resolve_config_dir()
    file_paths = resolve_config_files_in_dir(config_dir)
    if not file_paths:
        raise ConfigNotFoundError(get_config_search_dirs())
    cookie_path = file_paths["cookie"]
    return read_cookie_data(cookie_path)


def get_magic(cookie_data=None):
    """从 cookie 中提取 biz_magic 值。"""
    if cookie_data is None:
        cookie_data = get_cookie()

    magic = extract_biz_magic_from_cookie(cookie_data)
    if magic:
        return magic

    raise RuntimeError("未在 cookie 中找到 biz_magic 值。")


# ---------------------------------------------------------------------------
# 批量输入工具
# ---------------------------------------------------------------------------


def parse_batch_input(raw_text):
    """解析批量输入，支持空格、英文逗号、中文逗号和换行。"""
    return [
        stripped
        for item in re.split(r"[\s,，]+", raw_text.strip())
        if (stripped := item.strip())
    ]


def normalize_batch_text(raw_text):
    """将批量输入规范化为一行一个值。"""
    return "\n".join(parse_batch_input(raw_text))
