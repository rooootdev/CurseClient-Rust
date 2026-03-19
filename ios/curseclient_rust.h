#pragma once

#ifdef __cplusplus
extern "C" {
#endif

char *cc_getmodslistjson(const char *query);
char *cc_getmodfilesjson(const char *dllink);
void cc_free_string(char *s);

#ifdef __cplusplus
}
#endif
