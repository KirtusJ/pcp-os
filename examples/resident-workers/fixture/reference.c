/* Hosted reference for the pinned upstream llama2.c implementation. */
#define TESTING
#include "run.c"

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    Transformer model;
    build_transformer(&model, argv[1]);
    puts("[");
    for (int prompt = 0; prompt < 16; ++prompt) {
        forward(&model, 1, 0);
        int token = 300 + prompt;
        printf("  [");
        for (int pos = 1; pos <= 3; ++pos) {
            float *logits = forward(&model, token, pos);
            token = 0;
            for (int i = 1; i < model.config.vocab_size; ++i) {
                if (logits[i] > logits[token]) token = i;
            }
            printf("%s%d", pos == 1 ? "" : ", ", token);
        }
        puts(prompt == 15 ? "]" : "],");
    }
    puts("]");
    free_transformer(&model);
    return 0;
}
