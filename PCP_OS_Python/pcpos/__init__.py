"""PCP OS: a record and enforcement layer for Adversarial Claim Review (ACR).

Standard library only. This package implements one stated subset of ACR's
requirements by mechanism and records the rest from observation. It makes no
finding true; it authenticates nobody; a pass of any check here is a local
fact about files. See the README beside this package.
"""
VERSION = "1.0"
RECORD_PROFILE = "ACR-OS-RECORD-1"
WORKSPACE_PROFILE = "ACR-OS-WORKSPACE-1"
BUNDLE_PROFILE = "ACR-OS-BUNDLE-1"
DEFAULT_LABEL = "PCP OS"
