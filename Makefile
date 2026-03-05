# Complier + Flags
CXX = g++

# O3 --> this is for aggressive optimization; tells complier to optimize for speed
# Wall --> show all warnings
# std=c++17 --> have the modern c++ features
# Iinclude -- > look for headers in the /include folder
CXXFLAGS = -std=c++17 -O3 -Wall -Iinclude

# directories
SRCDIR = src
OBJDIR = obj
BINDIR = bin

# find all .cpp files in src/
SOURCES = $(wildcard $(SRCDIR)/*.cpp)

# convert src/file.cpp into obj/fil.o
OBJECTS = $(SOURCES:$(SRCDIR)/%.cpp=$(OBJDIR)/%.o)
TARGET = $(BINDIR)/oes_engine

# rules
all: $(TARGET)

# link objects to final binary
$(TARGET): $(OBJECTS)
	@mkdir -p $(BINDIR)
	$(CXX) $(CXXFLAGS) $(OBJECTS) -o $(TARGET)

# complie each .cpp into a .o
$(OBJDIR)/%.o: $(SRCDIR)/%.cpp
	@mkdir -p $(OBJDIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

# clean build files
clean:
	rm -rf $(OBJDIR) $(BINDIR)

.PHONY: all clean