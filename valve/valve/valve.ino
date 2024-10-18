#include <Servo.h>

Servo valveServo1;
Servo valveServo2;
const int MIN_POS = 115;
const int MAX_POS = 180;
const int CLOSE_POS = 180;
int desiredPosition = MIN_POS;

void setup() {
  valveServo1.attach(2);
  valveServo2.attach(3);
  valveServo1.write(MIN_POS); // Initialize to safe position
  valveServo2.write(MIN_POS); // Initialize to safe position
  Serial.begin(115200);
}

void loop() {
  Serial.println("Enter position for both servos: ");
  while (Serial.available() == 0) {};
  desiredPosition = Serial.readString().toInt();
  
  // Safety checks before moving the servos
  if (isSafeToMove()) {
    int constrainedPosition = constrain(desiredPosition, MIN_POS, MAX_POS);
    valveServo1.write(constrainedPosition);
    valveServo2.write(constrainedPosition);
    Serial.print("Both valve servos turned to: ");
    Serial.println(constrainedPosition);
  } else {
    // Take appropriate action (e.g., shut down)
    emergencyShutdown();
  }

  delay(1000);
}

bool isSafeToMove() {
  // Implement safety logic
  return true; // Placeholder
}

void emergencyShutdown() {
  valveServo1.write(CLOSE_POS); // Move to safe position
  valveServo2.write(CLOSE_POS); // Move to safe position
  Serial.println("Emergency shutdown initiated!");
  // Additional shutdown procedures
}
