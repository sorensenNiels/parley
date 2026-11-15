-- title: Passwords that expire soon
-- tags: did you know, security, users
-- about: User profiles whose password expires within some days (QSYS2.USER_INFO). Shows only the profiles you may see.
-- needs: IBM i 7.2
SELECT AUTHORIZATION_NAME, TEXT_DESCRIPTION, STATUS, DATE_PASSWORD_EXPIRES
  FROM QSYS2.USER_INFO
  WHERE DATE_PASSWORD_EXPIRES BETWEEN CURRENT TIMESTAMP AND CURRENT TIMESTAMP + ${days:14} DAYS
  ORDER BY DATE_PASSWORD_EXPIRES
