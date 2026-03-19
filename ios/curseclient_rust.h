#pragma once

#ifdef __cplusplus
extern "C" {
#endif

char *ccgetmodslistjson(const char *query);
char *ccgetmodfilesjson(const char *dllink);
void ccfreestring(char *s);

#ifdef __cplusplus
}
#endif
