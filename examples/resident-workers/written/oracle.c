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
    char decoded[1024];
    size_t used = 0;
    int previous = tokens[count - 1];
    const char *stop = "CAP";
    printf("OUTPUT");
    for (int i = 0; i < 48; ++i) {
        int id = sample_argmax(logits, model.config.vocab_size);
        printf(" %d", id);
        if (id == 2) { stop = "EOS"; break; }
        char *piece = decode(&vocabulary, previous, id);
        size_t length = strlen(piece);
        if (length > sizeof(decoded) - used) return 2;
        memcpy(decoded + used, piece, length);
        used += length;
        previous = id;
        if (i < 47) logits = forward(&model, id, count + i);
    }
    printf("\nSTOP %s\nDECODED_HEX ", stop);
    for (size_t i = 0; i < used; ++i) printf("%02x", (unsigned char)decoded[i]);
    putchar('\n');
    free_tokenizer(&vocabulary);
    free_transformer(&model);
    return 0;
}
