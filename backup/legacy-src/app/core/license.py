#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""授权兼容层：对外保留旧 API，内部委托给安全运行时。"""

from __future__ import annotations

from typing import Optional, Tuple

from core.security_runtime import (
    activate_license,
    authorize_task,
    check_stored_license,
    check_stored_license_local,
    deactivate_license,
    get_device_id,
    get_license_info,
    issue_or_refresh_session_token,
    load_runtime_state,
    refresh_lease_if_due,
    validate_runtime_continuity,
    verify_signed_lease as verify_signed_claims,
)

__all__ = [
    "activate_license",
    "authorize_task",
    "check_stored_license",
    "check_stored_license_local",
    "deactivate_license",
    "get_device_id",
    "get_license_info",
    "issue_or_refresh_session_token",
    "load_runtime_state",
    "refresh_lease_if_due",
    "validate_runtime_continuity",
    "verify_signed_claims",
]
