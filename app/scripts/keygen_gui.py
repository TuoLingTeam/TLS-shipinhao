#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""TLS-shipinhao 卡密生成器（管理员专用）。"""

import os
import sys
import tkinter as tk
from datetime import datetime
from tkinter import filedialog, font as tkfont

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from src.license import PLAN_DAYS, generate_key  # noqa: E402

_FAMILY = "PingFang SC" if sys.platform == "darwin" else "Microsoft YaHei UI"
_MONO = "Menlo" if sys.platform == "darwin" else "Consolas"

_PALETTE = {
    "bg": "#1A1B2E",
    "panel": "#242640",
    "border": "#3D3F5C",
    "entry_bg": "#1E1F34",
    "t1": "#F1F5F9",
    "t2": "#A5B4FC",
    "t3": "#7C8DB5",
    "accent": "#7C3AED",
    "acc_hov": "#6D28D9",
    "acc_txt": "#FFFFFF",
    "ghost_bg": "#2A2C48",
    "ghost_fg": "#22D3EE",
    "ghost_hov": "#353760",
    "ok": "#34D399",
}


class KeygenGUI:
    """管理员卡密生成工具。"""

    def __init__(self):
        colors = _PALETTE
        self.root = tk.Tk()
        self.root.title("TLS-shipinhao 卡密生成器（管理员）")
        self.root.geometry("620x560")
        self.root.resizable(False, False)
        self.root.configure(bg=colors["bg"])
        self._center()

        title_font = tkfont.Font(family=_FAMILY, size=18, weight="bold")
        body_font = tkfont.Font(family=_FAMILY, size=11)
        small_font = tkfont.Font(family=_FAMILY, size=10)
        mono_font = tkfont.Font(family=_MONO, size=12)
        btn_font = tkfont.Font(family=_FAMILY, size=11, weight="bold")

        outer = tk.Frame(self.root, bg=colors["bg"])
        outer.pack(fill=tk.BOTH, expand=True, padx=30, pady=24)

        tk.Label(
            outer,
            text="TLS-shipinhao 卡密生成器",
            bg=colors["bg"],
            fg=colors["t1"],
            font=title_font,
        ).pack(anchor=tk.W)
        tk.Label(
            outer,
            text="管理员专用：批量生成离线激活卡密",
            bg=colors["bg"],
            fg=colors["t2"],
            font=small_font,
        ).pack(anchor=tk.W, pady=(4, 16))

        config_panel = tk.Frame(
            outer,
            bg=colors["panel"],
            highlightbackground=colors["border"],
            highlightthickness=1,
        )
        config_panel.pack(fill=tk.X)
        config_inner = tk.Frame(config_panel, bg=colors["panel"])
        config_inner.pack(fill=tk.X, padx=16, pady=14)

        row1 = tk.Frame(config_inner, bg=colors["panel"])
        row1.pack(fill=tk.X, pady=(0, 8))
        tk.Label(row1, text="有效期：", bg=colors["panel"], fg=colors["t1"], font=body_font).pack(side=tk.LEFT)
        tk.Label(
            row1,
            text=f"{PLAN_DAYS} 天（固定）",
            bg=colors["panel"],
            fg=colors["accent"],
            font=tkfont.Font(family=_FAMILY, size=11, weight="bold"),
        ).pack(side=tk.LEFT, padx=(4, 0))

        row2 = tk.Frame(config_inner, bg=colors["panel"])
        row2.pack(fill=tk.X)
        tk.Label(row2, text="生成数量：", bg=colors["panel"], fg=colors["t1"], font=body_font).pack(side=tk.LEFT)

        qty_border = tk.Frame(row2, bg=colors["border"])
        qty_border.pack(side=tk.LEFT, padx=(4, 0))
        self._qty_var = tk.StringVar(value="10")
        qty_entry = tk.Entry(
            qty_border,
            textvariable=self._qty_var,
            width=6,
            bg=colors["entry_bg"],
            fg=colors["t1"],
            insertbackground=colors["t1"],
            relief=tk.FLAT,
            borderwidth=0,
            font=body_font,
            justify="center",
        )
        qty_entry.pack(padx=1, pady=1, ipady=2)

        tk.Label(row2, text="（1~100）", bg=colors["panel"], fg=colors["t3"], font=small_font).pack(
            side=tk.LEFT,
            padx=(6, 0),
        )

        btn_row = tk.Frame(outer, bg=colors["bg"])
        btn_row.pack(fill=tk.X, pady=(12, 0))

        gen_border = tk.Frame(btn_row, bg=colors["acc_hov"])
        gen_border.pack(side=tk.LEFT, fill=tk.X, expand=True, padx=(0, 6))
        gen_btn = tk.Label(
            gen_border,
            text="生成卡密",
            font=btn_font,
            fg=colors["acc_txt"],
            bg=colors["accent"],
            cursor="hand2",
            anchor="center",
        )
        gen_btn.pack(fill=tk.X, ipady=6, padx=1, pady=1)
        gen_btn.bind("<Button-1>", lambda _e: self._generate())
        gen_btn.bind("<Enter>", lambda _e: gen_btn.configure(bg=colors["acc_hov"]))
        gen_btn.bind("<Leave>", lambda _e: gen_btn.configure(bg=colors["accent"]))

        copy_border = tk.Frame(btn_row, bg=colors["border"])
        copy_border.pack(side=tk.LEFT, padx=(0, 6))
        copy_btn = tk.Label(
            copy_border,
            text="复制全部",
            font=body_font,
            fg=colors["ghost_fg"],
            bg=colors["ghost_bg"],
            cursor="hand2",
            anchor="center",
        )
        copy_btn.pack(ipady=6, ipadx=12, padx=1, pady=1)
        copy_btn.bind("<Button-1>", lambda _e: self._copy_all())
        copy_btn.bind("<Enter>", lambda _e: copy_btn.configure(bg=colors["ghost_hov"]))
        copy_btn.bind("<Leave>", lambda _e: copy_btn.configure(bg=colors["ghost_bg"]))

        export_border = tk.Frame(btn_row, bg=colors["border"])
        export_border.pack(side=tk.LEFT)
        export_btn = tk.Label(
            export_border,
            text="导出 TXT",
            font=body_font,
            fg=colors["ghost_fg"],
            bg=colors["ghost_bg"],
            cursor="hand2",
            anchor="center",
        )
        export_btn.pack(ipady=6, ipadx=12, padx=1, pady=1)
        export_btn.bind("<Button-1>", lambda _e: self._export())
        export_btn.bind("<Enter>", lambda _e: export_btn.configure(bg=colors["ghost_hov"]))
        export_btn.bind("<Leave>", lambda _e: export_btn.configure(bg=colors["ghost_bg"]))

        self._status_var = tk.StringVar()
        self._status_label = tk.Label(
            outer,
            textvariable=self._status_var,
            bg=colors["bg"],
            fg=colors["ok"],
            font=small_font,
            anchor="w",
        )
        self._status_label.pack(anchor=tk.W, pady=(8, 0))

        result_border = tk.Frame(outer, bg=colors["border"])
        result_border.pack(fill=tk.BOTH, expand=True, pady=(8, 0))
        self._text = tk.Text(
            result_border,
            bg=colors["entry_bg"],
            fg=colors["t1"],
            insertbackground=colors["t1"],
            font=mono_font,
            relief=tk.FLAT,
            borderwidth=0,
            padx=12,
            pady=10,
        )
        self._text.pack(fill=tk.BOTH, expand=True, padx=1, pady=1)

    def _center(self):
        self.root.update_idletasks()
        width, height = 620, 560
        x = (self.root.winfo_screenwidth() - width) // 2
        y = (self.root.winfo_screenheight() - height) // 2
        self.root.geometry(f"{width}x{height}+{x}+{y}")

    def _generate(self):
        try:
            qty = int(self._qty_var.get().strip())
        except ValueError:
            self._status_var.set("请输入有效数字")
            self._status_label.configure(fg=_PALETTE["t1"])
            return

        qty = max(1, min(qty, 100))
        self._qty_var.set(str(qty))
        keys = [generate_key() for _ in range(qty)]

        self._text.delete("1.0", tk.END)
        self._text.insert("1.0", "\n".join(keys))
        self._status_var.set(f"已生成 {qty} 个卡密（有效期 {PLAN_DAYS} 天）")
        self._status_label.configure(fg=_PALETTE["ok"])

    def _copy_all(self):
        content = self._text.get("1.0", tk.END).strip()
        if not content:
            self._status_var.set("没有可复制的内容")
            return
        self.root.clipboard_clear()
        self.root.clipboard_append(content)
        self._status_var.set("已复制到剪贴板")
        self._status_label.configure(fg=_PALETTE["ok"])

    def _export(self):
        content = self._text.get("1.0", tk.END).strip()
        if not content:
            self._status_var.set("没有可导出的内容")
            return
        ts = datetime.now().strftime("%Y%m%d_%H%M%S")
        path = filedialog.asksaveasfilename(
            title="导出卡密",
            defaultextension=".txt",
            initialfile=f"tls_shipinhao_keys_{ts}.txt",
            filetypes=[("Text files", "*.txt")],
        )
        if not path:
            return
        with open(path, "w", encoding="utf-8") as file:
            file.write(content + "\n")
        self._status_var.set(f"已导出到 {path}")
        self._status_label.configure(fg=_PALETTE["ok"])

    def run(self):
        self.root.mainloop()


if __name__ == "__main__":
    KeygenGUI().run()
