"""Candidate firmware tests use the 1024×768 Details view for comparable register captures."""

OPTIONS = "first=details desktop=1024x768"

# The selected region on the current Details screen includes the D desktop and P off
# hints. The historical register footer and both sequence-number column hashes still match.
FOOTER = (12, 735, 895, 760, "2f75c23ac365a0a4a3a70a0ac590b1f3892b1c471b68dd71b1f21d8751524888")
