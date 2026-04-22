// Compile with: gcc get_active_game.c -o get_active_game
// Run with: ./get_active_game

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/un.h>

#define SOCKET_PATH "/run/contextd/public/contextd.socket"

// Note: String literals in C implicitly have a null terminator ('\0') at the end.
// We use sizeof() which includes this null terminator because Varlink requires it.
#define REQUEST "{\"method\":\"com.performativenonsense.contextd.GetActiveGame\",\"parameters\":{}}"

int main() {
    int sock = socket(AF_UNIX, SOCK_STREAM, 0);
    if (sock < 0) {
        perror("socket");
        return 1;
    }

    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, SOCKET_PATH, sizeof(addr.sun_path) - 1);

    if (connect(sock, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        perror("connect");
        fprintf(stderr, "Is the contextd daemon running?\n");
        close(sock);
        return 1;
    }

    // Send the varlink request (including the null byte)
    if (write(sock, REQUEST, sizeof(REQUEST)) < 0) {
        perror("write");
        close(sock);
        return 1;
    }

    // Read the response until we hit the null byte
    char buffer[4096];
    int bytes_read;
    printf("Raw JSON Response:\n");
    
    while ((bytes_read = read(sock, buffer, sizeof(buffer))) > 0) {
        for (int i = 0; i < bytes_read; i++) {
            if (buffer[i] == '\0') {
                printf("\n");
                close(sock);
                return 0;
            } else {
                putchar(buffer[i]);
            }
        }
    }

    close(sock);
    return 0;
}
