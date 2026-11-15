-- title: Users and their last sign-on
-- tags: did you know, security
-- about: User profiles, their status, last sign-on and failed sign-on attempts (QSYS2.USER_INFO).
-- needs: IBM i 7.1
SELECT AUTHORIZATION_NAME, TEXT_DESCRIPTION, STATUS, PREVIOUS_SIGNON, SIGN_ON_ATTEMPTS_NOT_VALID
  FROM QSYS2.USER_INFO
  ORDER BY PREVIOUS_SIGNON DESC
