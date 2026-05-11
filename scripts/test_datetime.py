import subprocess, json

# Test DATE/TIME/TIMESTAMP/INTERVAL via the Go driver's dmcs
queries = [
    "SELECT CURRENT_DATE, CURRENT_TIME, CURRENT_TIMESTAMP FROM DUAL",
    "SELECT SYSDATE FROM DUAL",
    "SELECT TO_DATE('2024-06-15', 'YYYY-MM-DD') AS d FROM DUAL",
    "SELECT TO_TIMESTAMP('2024-06-15 10:30:00.123456', 'YYYY-MM-DD HH24:MI:SS.FF') AS ts FROM DUAL",
]

for q in queries:
    print(f"Query: {q}")
    print("---")
