-- title: Network connections
-- tags: did you know, network
-- about: TCP connections to and from this system, as NETSTAT shows them (QSYS2.NETSTAT_INFO).
-- needs: IBM i 7.2
SELECT REMOTE_ADDRESS, REMOTE_PORT, LOCAL_ADDRESS, LOCAL_PORT, TCP_STATE, IDLE_TIME
  FROM QSYS2.NETSTAT_INFO
  ORDER BY LOCAL_PORT, REMOTE_ADDRESS
