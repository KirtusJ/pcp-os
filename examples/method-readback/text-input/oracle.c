/* The pinned upstream tokenizer and transformer run independently on the host. */
#define TESTING
#include "run.c"
#include <inttypes.h>

int main(int argc, char **argv) {
    if (argc != 4) return 2;
    size_t bytes = strlen(argv[3]);
    if (bytes == 0 || bytes > 92) return 2;
    Transformer model;
    Tokenizer vocabulary;
    build_transformer(&model, argv[1]);
    build_tokenizer(&vocabulary, argv[2], model.config.vocab_size);
    int tokens[96], count = 0;
    encode(&vocabulary, argv[3], 1, 0, tokens, &count);
    printf("INPUT");
    for (int pos = 0; pos < count; ++pos) printf(" %d", tokens[pos]);
    putchar('\n');
    float *logits = NULL;
    for (int pos = 0; pos < count; ++pos) logits = forward(&model, tokens[pos], pos);
    uint32_t prompt_scores[512];
    if (model.config.vocab_size != 512) return 2;
    memcpy(prompt_scores, logits, sizeof prompt_scores);
    printf("OUTPUT");
    for (int n = 0; n < 3; ++n) {
        int id = sample_argmax(logits, model.config.vocab_size);
        printf(" %d", id);
        if (n < 2) logits = forward(&model, id, count + n);
    }
    putchar('\n');
    for (int i = 0; i < 512; i += 16) {
        printf("LOGITS");
        for (int j = 0; j < 16; ++j) printf(" %08" PRIx32, prompt_scores[i + j]);
        putchar('\n');
    }
    free_tokenizer(&vocabulary);
    free_transformer(&model);
    return 0;
}
