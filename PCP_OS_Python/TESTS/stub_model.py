"""A stand-in for a model, for tests: reads one JSON object on standard input and prints one on standard output.

It reports what it was given, so that a test can see exactly what reached the model.
"""
import json, sys
req = json.loads(sys.stdin.read())
system = req.get("system") or ""
last = req["messages"][-1]["content"]
text = "kernel=%s data_blocks=%d turns=%d last=%s" % ("yes" if "ACR Kernel" in system or "ACR card" in system else "no",
                                                    sum(m["content"].count("[DATA from") for m in req["messages"]),
                                                    len(req["messages"]), last[:60].replace("\n", " "))
print(json.dumps({"text": text, "reasoning": "considered the request" if "trace please" in last else None,
                  "usage": {"input_tokens": len(json.dumps(req)) // 4, "output_tokens": 12, "ratio": 0.5}, "model": "stub-1"}))
